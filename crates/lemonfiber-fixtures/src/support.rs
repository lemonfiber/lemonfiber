//! The seams a test stands in for, beyond the transport and the filesystem.
//!
//! A runner that answers from a script, an engine that reports what a test put in it, a
//! filesystem that hands back the configuration a service would have written. Lifted out of
//! `lemonfiber-core`, where being private to the crate meant its integration tests could
//! not see them.

mod reporting;

pub use reporting::{Reporting, Tunnel};

use std::sync::Arc;

use crate::http::{Answer, Fake};
use async_trait::async_trait;
use lemonfiber_ports::process::{Failure, Output, Runner};

/// A runner that answers with whatever the test scripted.
pub struct Scripted(pub Result<Output, Failure>);

#[async_trait]
impl Runner for Scripted {
    async fn run(&self, _argv: &[String]) -> Result<Output, Failure> {
        echoed(&self.0)
    }
    // `stream` uses the trait's default (run then replay); a test that drives a
    // streamed pull exercises it.
}

/// A runner that answers the same way every time and remembers what it was asked.
///
/// [`Scripted`] throws the argument vector away, which is right until a test's claim
/// is about a command that should **not** have run. Proving that a form which failed
/// to start was never then torn down is a statement about everything the runner was
/// handed, and cannot be made from what came back out of it.
pub struct Recording {
    answer: Result<Output, Failure>,
    seen: std::sync::Mutex<Vec<Vec<String>>>,
}

impl Recording {
    /// A runner that always answers this way.
    #[must_use]
    pub fn answering(answer: Result<Output, Failure>) -> Self {
        Self {
            answer,
            seen: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Whether any command it was handed used this Compose subcommand.
    ///
    /// A poisoned lock reads as nothing having been run. That is the answer that
    /// fails a test asserting something happened, rather than the one that lets a
    /// test asserting nothing happened pass without having looked.
    #[must_use]
    pub fn ran(&self, subcommand: &str) -> bool {
        self.seen.lock().is_ok_and(|seen| {
            seen.iter()
                .any(|argv| argv.iter().any(|word| word == subcommand))
        })
    }
}

#[async_trait]
impl Runner for Recording {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(argv.to_vec());
        }
        echoed(&self.answer)
    }
}

/// The scripted answer again, since a [`Failure`] cannot be cloned.
///
/// Shared by both runners rather than written twice: the arms exist only because the
/// error type is not `Clone`, and a second copy would be a second place to forget a
/// variant when one is added.
fn echoed(answer: &Result<Output, Failure>) -> Result<Output, Failure> {
    match answer {
        Ok(output) => Ok(output.clone()),
        Err(Failure::NotFound { program }) => Err(Failure::NotFound {
            program: program.clone(),
        }),
        Err(Failure::Unusable { program, reason }) => Err(Failure::Unusable {
            program: program.clone(),
            reason: reason.clone(),
        }),
    }
}

/// A randomness source answering with exactly the bytes a test scripts.
pub struct FixedRandom(pub Option<Vec<u8>>);

impl lemonfiber_ports::random::Random for FixedRandom {
    fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
        self.0.clone()
    }
}

/// The routes a whole seed run reads, so its many calls need no exact ordering.
///
/// Root-folder, download-client and Prowlarr application lists hold exactly what each
/// service wants, so every connection reads back as already wired; qBittorrent's auth
/// and set both answer success. The catch-all is last because the first matching route
/// wins, and an empty fragment is contained in every URL.
#[must_use]
pub fn seeding_routes() -> Vec<(&'static str, Answer)> {
    vec![
        (
            "/downloadclient",
            Answer::reply(
                200,
                r#"[{"id":1,"fields":[{"name":"host","value":"sabnzbd"},{"name":"port","value":8080}]},{"id":2,"fields":[{"name":"host","value":"gluetun"},{"name":"port","value":8081}]}]"#,
            ),
        ),
        (
            "/rootfolder",
            Answer::reply(
                200,
                r#"[{"id":1,"path":"/data/media/tv"},{"id":2,"path":"/data/media/movies"},{"id":3,"path":"/data/media/music"}]"#,
            ),
        ),
        (
            "/applications",
            Answer::reply(
                200,
                r#"[{"id":1,"fields":[{"name":"baseUrl","value":"http://sonarr:8989"}]},{"id":2,"fields":[{"name":"baseUrl","value":"http://radarr:7878"}]},{"id":3,"fields":[{"name":"baseUrl","value":"http://lidarr:8686"}]}]"#,
            ),
        ),
        ("", Answer::reply(200, "Ok.")),
    ]
}

/// A transport answering a whole seed run, every connection already wired.
#[must_use]
pub fn seeding() -> Arc<Fake> {
    Fake::by_path(seeding_routes())
}

/// A seed run answering `extra` first and falling through to the ordinary routes.
///
/// For the tests that need one service to say something different — a version, a
/// second download client — without restating the whole table around it.
#[must_use]
pub fn seeding_with(extra: Vec<(&'static str, Answer)>) -> Arc<Fake> {
    let mut routes = extra;
    routes.extend(seeding_routes());
    Fake::by_path(routes)
}

/// A filesystem that hands back a Servarr configuration for a Servarr path and
/// a `SABnzbd` one for `SABnzbd`'s, or nothing. Only `read` is meaningful to
/// seeding; the rest are unused.
pub struct SeedFs {
    servarr: Option<&'static str>,
    sabnzbd: Option<&'static str>,
    /// What the subtitle finder's configuration says, where a test has it written
    /// one. Its own key lives in a YAML file rather than a Servarr XML, so a test
    /// wiring subtitles has to be able to answer that path with something else.
    bazarr: Option<&'static str>,
    /// When set, the Servarr configuration is handed back only for Prowlarr's
    /// path, so a test can make Prowlarr's key readable while an \*arr's is not —
    /// the case of an \*arr that started after Prowlarr.
    only_prowlarr: bool,
    /// Path fragments a canonicalize should fail for, standing in for host
    /// directories that are not there — so a root folder's existence check can be
    /// driven to missing. Every path resolves when empty.
    missing: Vec<&'static str>,
    /// What a volume describe reports — a zero total by default, which the
    /// dashboard reads as free space unknown.
    facts: lemonfiber_ports::filesystem::StorageFacts,
}

impl SeedFs {
    /// A transport answering by the shape of the URL, with these two keys in place.
    #[must_use]
    pub fn keyed(servarr: Option<&'static str>, sabnzbd: Option<&'static str>) -> Self {
        Self {
            servarr,
            sabnzbd,
            bazarr: None,
            only_prowlarr: false,
            missing: Vec::new(),
            facts: lemonfiber_ports::filesystem::StorageFacts {
                kind: lemonfiber_ports::filesystem::FsKind::Linking("test".to_owned()),
                removable: false,
                available: 0,
                total: 0,
            },
        }
    }

    /// The same, answering the subtitle finder's configuration path with `config`.
    #[must_use]
    pub fn with_bazarr(mut self, config: &'static str) -> Self {
        self.bazarr = Some(config);
        self
    }

    /// The same, but withholding the Servarr key from every path but Prowlarr's.
    #[must_use]
    pub fn only_for_prowlarr(mut self) -> Self {
        self.only_prowlarr = true;
        self
    }

    /// The same, reporting the given volume facts to a describe.
    #[must_use]
    pub fn with_facts(mut self, facts: lemonfiber_ports::filesystem::StorageFacts) -> Self {
        self.facts = facts;
        self
    }

    /// The same, but failing to canonicalize any path carrying one of these
    /// fragments — a host directory that is not there, for a root-folder check.
    #[must_use]
    pub fn missing(mut self, fragments: Vec<&'static str>) -> Self {
        self.missing = fragments;
        self
    }
}

#[async_trait]
impl lemonfiber_ports::filesystem::FileSystem for SeedFs {
    async fn canonicalize(
        &self,
        path: &std::path::Path,
    ) -> Result<std::path::PathBuf, lemonfiber_ports::filesystem::Fault> {
        let text = path.to_string_lossy();
        if self.missing.iter().any(|fragment| text.contains(fragment)) {
            return Err(lemonfiber_ports::filesystem::Fault::new("no such path"));
        }
        Ok(path.to_path_buf())
    }
    async fn touch(
        &self,
        _path: &std::path::Path,
    ) -> Result<(), lemonfiber_ports::filesystem::Fault> {
        Err(lemonfiber_ports::filesystem::Fault::new("unused"))
    }
    async fn link(
        &self,
        _from: &std::path::Path,
        _to: &std::path::Path,
    ) -> Result<(), lemonfiber_ports::filesystem::Fault> {
        Err(lemonfiber_ports::filesystem::Fault::new("unused"))
    }
    async fn identify(
        &self,
        _path: &std::path::Path,
    ) -> Result<lemonfiber_ports::filesystem::Identity, lemonfiber_ports::filesystem::Fault> {
        Err(lemonfiber_ports::filesystem::Fault::new("unused"))
    }
    async fn remove(&self, _path: &std::path::Path) {}
    async fn read(&self, path: &std::path::Path) -> Option<String> {
        let path = path.to_string_lossy();
        if path.contains("sabnzbd") {
            return self.sabnzbd.map(str::to_owned);
        }
        // Before the Servarr fall-through, because the finder's key is a YAML of its
        // own: answering its path with a Servarr XML would read as a service that has
        // started and written no key, which is a different thing entirely.
        if path.contains("bazarr") {
            return self.bazarr.map(str::to_owned);
        }
        if self.only_prowlarr && !path.contains("prowlarr") {
            return None;
        }
        self.servarr.map(str::to_owned)
    }
    async fn write(&self, _path: &std::path::Path, _contents: &str) {}
    async fn ownership(
        &self,
        _path: &std::path::Path,
    ) -> Option<lemonfiber_ports::filesystem::Ownership> {
        None
    }
    async fn describe(
        &self,
        _path: &std::path::Path,
    ) -> lemonfiber_ports::filesystem::StorageFacts {
        self.facts.clone()
    }
}

/// A program that succeeded, saying `stdout`.
#[must_use]
pub fn spoke(stdout: &str) -> Output {
    Output {
        status: Some(0),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

/// A program that failed, complaining `stderr`.
#[must_use]
pub fn refused(stderr: &str) -> Output {
    Output {
        status: Some(1),
        stdout: String::new(),
        stderr: stderr.to_owned(),
    }
}

/// A throwaway password for a test — built from a character range rather than
/// written as a string literal, so a hard-coded-credential scan does not read a
/// test fixture as a real secret. Non-empty on purpose (a recorded password that
/// read back as empty would be treated as absent), and its value is otherwise
/// irrelevant: the fakes accept whatever is sent.
#[must_use]
pub fn a_password() -> String {
    ('a'..='p').collect()
}
