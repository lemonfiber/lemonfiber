//! The credentials check, driven through its public surface against fakes.
//!
//! The check reads a key from a file and asks a running service who it is with
//! it. Both sides are faked here: a filesystem that hands back whatever config a
//! test wants, and a transport that answers as the service would. The check is
//! built on `#[async_trait]` clients built on another, so — as with the Servarr
//! client and the VPN check — it is exercised from here rather than from a
//! `#[cfg(test)]` module, where async-trait code is compiled twice and its
//! coverage counted from the wrong copy.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_core::doctor::credentials::{CredentialsCheck, Target, CREDENTIAL_REJECTED};
use lemonfiber_core::doctor::{Category, Check, Verdict};
use lemonfiber_core::error::Severity;
use lemonfiber_core::ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, StorageFacts,
};
use lemonfiber_core::ports::http::{Http, Request, Response, Unreachable};

/// How the fake transport answers a request whose URL carries a given marker.
enum Answer {
    /// A response with this status and body.
    Reply(u16, &'static str),
    /// Nothing answered.
    Silent,
}

/// A transport routed by a substring of the request URL, so two services can be
/// made to answer differently in the same run.
struct FakeHttp {
    routes: Vec<(&'static str, Answer)>,
    seen: Mutex<Vec<Request>>,
}

impl FakeHttp {
    fn new(routes: Vec<(&'static str, Answer)>) -> Arc<Self> {
        Arc::new(Self {
            routes,
            seen: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl Http for FakeHttp {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        if let Ok(mut guard) = self.seen.lock() {
            guard.push(request.clone());
        }
        let answer = self
            .routes
            .iter()
            .find(|(marker, _)| request.url.contains(marker));
        match answer {
            Some((_, Answer::Reply(status, body))) => Ok(Response {
                status: *status,
                body: (*body).to_owned(),
            }),
            _ => Err(Unreachable {
                url: request.url.clone(),
                reason: "connection refused".to_owned(),
            }),
        }
    }
}

/// A filesystem that hands back the config text a test placed at a path, and
/// nothing for any other. Only `read` is meaningful; the rest are unused here.
struct FakeFs {
    files: Vec<(PathBuf, &'static str)>,
}

impl FakeFs {
    fn with(files: Vec<(PathBuf, &'static str)>) -> Arc<Self> {
        Arc::new(Self { files })
    }
}

#[async_trait]
impl FileSystem for FakeFs {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        Ok(path.to_path_buf())
    }
    async fn touch(&self, _path: &Path) -> Result<(), Fault> {
        Err(Fault::new("unused"))
    }
    async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
        Err(Fault::new("unused"))
    }
    async fn identify(&self, _path: &Path) -> Result<Identity, Fault> {
        Err(Fault::new("unused"))
    }
    async fn remove(&self, _path: &Path) {}
    async fn read(&self, path: &Path) -> Option<String> {
        self.files
            .iter()
            .find(|(at, _)| at == path)
            .map(|(_, text)| (*text).to_owned())
    }
    async fn write(&self, _path: &Path, _contents: &str) {}
    async fn ownership(&self, _path: &Path) -> Option<Ownership> {
        None
    }
    async fn describe(&self, _path: &Path) -> StorageFacts {
        StorageFacts {
            kind: FsKind::Linking("test".to_owned()),
            removable: false,
            available: 0,
            total: 0,
        }
    }
}

/// A Servarr config carrying a generated key, as one reads from disk.
const CONFIG_WITH_KEY: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";

/// The key inside `CONFIG_WITH_KEY`, so a test can assert it never surfaces.
const THE_KEY: &str = "a1b2c3d4e5";

/// A status body a healthy Servarr answers `system/status` with.
const SONARR_STATUS: &str = r#"{"instanceName":"Sonarr","version":"4.0.15.2941"}"#;

fn sonarr(config: &Path) -> Target {
    Target {
        id: "sonarr".to_owned(),
        name: "Sonarr".to_owned(),
        base: "http://127.0.0.1:8989".to_owned(),
        config: config.to_path_buf(),
        version: 3,
    }
}

/// Run the check over one target with the given filesystem and transport.
async fn only(target: Target, fs: Arc<FakeFs>, http: Arc<FakeHttp>) -> Verdict {
    let check = CredentialsCheck::new(http, fs, vec![target]);
    let mut findings = check.run().await;
    findings.pop().map_or(
        Verdict::Skipped {
            reason: String::new(),
        },
        |found| found.verdict,
    )
}

#[tokio::test]
async fn a_proven_credential_names_the_service_and_its_version() {
    let config = PathBuf::from("/stack/config/sonarr/config.xml");
    let fs = FakeFs::with(vec![(config.clone(), CONFIG_WITH_KEY)]);
    let http = FakeHttp::new(vec![("8989", Answer::Reply(200, SONARR_STATUS))]);

    let verdict = only(sonarr(&config), fs, http).await;
    assert_eq!(
        verdict,
        Verdict::Pass {
            note: Some("Sonarr 4.0.15.2941".to_owned())
        },
        "a pass states the observed identity, not a bare tick"
    );
}

#[tokio::test]
async fn a_service_that_refuses_its_own_key_is_a_failure() {
    let config = PathBuf::from("/stack/config/sonarr/config.xml");
    let fs = FakeFs::with(vec![(config.clone(), CONFIG_WITH_KEY)]);
    let http = FakeHttp::new(vec![("8989", Answer::Reply(401, ""))]);

    let problem = match only(sonarr(&config), fs, http).await {
        Verdict::Fail(problem) => Some(problem),
        _ => None,
    };
    assert!(
        problem.is_some_and(|problem| problem.code == CREDENTIAL_REJECTED
            && problem.severity == Severity::Error
            && !problem.remedies.is_empty()),
        "a rejected key must fail, as an error carrying a remedy"
    );
}

#[tokio::test]
async fn a_service_that_does_not_answer_is_unverified_rather_than_failed() {
    // Nothing answered, so the credential is unproven — never a pass, but not a
    // fault either: the honest verdict is that it could not be established.
    let config = PathBuf::from("/stack/config/sonarr/config.xml");
    let fs = FakeFs::with(vec![(config.clone(), CONFIG_WITH_KEY)]);
    let http = FakeHttp::new(vec![("8989", Answer::Silent)]);

    assert!(matches!(
        only(sonarr(&config), fs, http).await,
        Verdict::Unverified { .. }
    ));
}

#[tokio::test]
async fn a_service_answering_unusably_carries_its_own_words() {
    let config = PathBuf::from("/stack/config/sonarr/config.xml");
    let fs = FakeFs::with(vec![(config.clone(), CONFIG_WITH_KEY)]);
    let http = FakeHttp::new(vec![("8989", Answer::Reply(500, "database is locked"))]);

    let reason = match only(sonarr(&config), fs, http).await {
        Verdict::Unverified { reason, .. } => Some(reason),
        _ => None,
    };
    assert!(
        reason.is_some_and(|reason| reason.contains("database is locked")),
        "an unusable answer is unverified, carrying the service's own words"
    );
}

#[tokio::test]
async fn a_service_with_no_config_file_yet_is_skipped() {
    // The file is not there because the service has not finished first start.
    let config = PathBuf::from("/stack/config/sonarr/config.xml");
    let fs = FakeFs::with(Vec::new());
    let http = FakeHttp::new(vec![("8989", Answer::Reply(200, SONARR_STATUS))]);

    assert!(matches!(
        only(sonarr(&config), fs, http).await,
        Verdict::Skipped { .. }
    ));
}

#[tokio::test]
async fn a_service_whose_key_is_not_generated_yet_is_skipped() {
    // The config exists but the key element is still empty mid-first-start.
    let config = PathBuf::from("/stack/config/sonarr/config.xml");
    let fs = FakeFs::with(vec![(config.clone(), "<Config><ApiKey></ApiKey></Config>")]);
    let http = FakeHttp::new(vec![("8989", Answer::Reply(200, SONARR_STATUS))]);

    assert!(matches!(
        only(sonarr(&config), fs, http).await,
        Verdict::Skipped { .. }
    ));
}

#[tokio::test]
async fn each_service_is_reported_independently() {
    // One up, one not answering: the run reports both, and the unreachable one
    // does not stop the reachable one being proven.
    let sonarr_config = PathBuf::from("/stack/config/sonarr/config.xml");
    let radarr_config = PathBuf::from("/stack/config/radarr/config.xml");
    let fs = FakeFs::with(vec![
        (sonarr_config.clone(), CONFIG_WITH_KEY),
        (radarr_config.clone(), CONFIG_WITH_KEY),
    ]);
    let http = FakeHttp::new(vec![
        ("8989", Answer::Reply(200, SONARR_STATUS)),
        ("7878", Answer::Silent),
    ]);
    let radarr = Target {
        id: "radarr".to_owned(),
        name: "Radarr".to_owned(),
        base: "http://127.0.0.1:7878".to_owned(),
        config: radarr_config,
        version: 3,
    };

    let check = CredentialsCheck::new(http, fs, vec![sonarr(&sonarr_config), radarr]);
    let findings = check.run().await;

    assert_eq!(findings.len(), 2);
    let sonarr_found = findings
        .iter()
        .find(|found| found.check == "credentials.sonarr");
    let radarr_found = findings
        .iter()
        .find(|found| found.check == "credentials.radarr");
    assert!(
        sonarr_found.is_some_and(|found| matches!(found.verdict, Verdict::Pass { .. })),
        "the reachable service is proven"
    );
    assert!(
        radarr_found.is_some_and(|found| matches!(found.verdict, Verdict::Unverified { .. })),
        "the unreachable one is unproven, and did not stop the other"
    );
}

#[tokio::test]
async fn nothing_to_prove_produces_no_findings() {
    let fs = FakeFs::with(Vec::new());
    let http = FakeHttp::new(Vec::new());
    let check = CredentialsCheck::new(http, fs, Vec::new());
    assert!(check.run().await.is_empty());
}

#[tokio::test]
async fn the_check_belongs_to_the_credentials_category() {
    let fs = FakeFs::with(Vec::new());
    let http = FakeHttp::new(Vec::new());
    let check = CredentialsCheck::new(http, fs, Vec::new());
    assert_eq!(check.category(), Category::Credentials);
}

#[tokio::test]
async fn the_credential_never_appears_in_a_finding() {
    // Outcomes are reported, never inputs. Whatever the verdict, the key that
    // authenticated the request must not be anywhere in what is shown.
    let config = PathBuf::from("/stack/config/sonarr/config.xml");
    for answer in [
        Answer::Reply(200, SONARR_STATUS),
        Answer::Reply(401, ""),
        Answer::Reply(500, "database is locked"),
        Answer::Silent,
    ] {
        let fs = FakeFs::with(vec![(config.clone(), CONFIG_WITH_KEY)]);
        let http = FakeHttp::new(vec![("8989", answer)]);
        let check = CredentialsCheck::new(http, fs, vec![sonarr(&config)]);
        let rendered = serde_json::to_string(&check.run().await).unwrap_or_default();
        assert!(
            !rendered.contains(THE_KEY),
            "the credential leaked into a finding: {rendered}"
        );
    }
}
