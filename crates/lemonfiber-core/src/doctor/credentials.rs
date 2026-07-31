//! Proving each service still answers to the credential lemonfiber holds.
//!
//! A wrong or stale credential is the quietest failure a stack has: everything
//! reports healthy and searches simply come back empty, weeks apart from the
//! cause. So this does not check the key's shape — it reads the key each service
//! wrote to its own configuration, asks the running service who it is with that
//! key, and reports what it observed. A proven credential names the service and
//! its version; there is no green tick without a fact behind it.
//!
//! The three ways it can go wrong are kept apart, because collapsing them sends
//! the operator after the wrong problem. A service that answers and refuses the
//! key is a fault to fix; one that does not answer at all leaves the credential
//! unproven rather than broken; one that answers unusably is carried through in
//! the service's own words rather than guessed at.
//!
//! The credential itself never leaves this module — not into a finding, a log or
//! a bundle. What is reported is the outcome, never the input.
//!
//! See `.docs/architecture/module-layout.md`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity};
use crate::ports::filesystem::FileSystem;
use crate::ports::http::Http;
use crate::ports::service::{Client, Failure};
use crate::servarr::{api_key, Servarr};

/// Raised when a service answers and refuses the credential it generated itself.
pub const CREDENTIAL_REJECTED: Code = Code::new("CRED-1");

/// One service whose credential is to be proven.
///
/// The address the host reaches it on and the host path to the file it wrote its
/// key to are resolved above, from the manifest and the running stack, so the
/// check stays a matter of reading a file and asking a question.
pub struct Target {
    /// The compose service id, such as `sonarr`; names the finding.
    pub id: String,
    /// The human-facing name, such as `Sonarr`; what the operator reads.
    pub name: String,
    /// The base URL the host reaches it on, such as `http://127.0.0.1:8989`.
    pub base: String,
    /// The host path to the configuration file holding the generated key.
    pub config: PathBuf,
    /// The major version of its API — the `/api/vN` segment. Sonarr and Radarr
    /// are v3, Lidarr and Prowlarr v1, so it travels with the target rather than
    /// being assumed.
    pub version: u32,
}

impl Target {
    /// A client for this service, or nothing where its config or key cannot be read
    /// yet — the "still starting, try again later" case every caller skips the same
    /// way (a missing key is a service that has not written it, not a fault). The
    /// read-back and the build are one step, so the callers that open a
    /// Servarr-shape service — the credentials check, the dashboard's queues, and
    /// seeding — cannot drift on how they do it.
    pub(crate) async fn open(&self, http: &Arc<dyn Http>, fs: &dyn FileSystem) -> Option<Servarr> {
        let config = fs.read(&self.config).await?;
        let key = api_key(&config)?;
        Some(Servarr::new(
            http.clone(),
            &self.base,
            key,
            &self.id,
            self.version,
        ))
    }
}

/// Proves each Servarr-shape service still answers to the key it wrote.
pub struct CredentialsCheck {
    http: Arc<dyn Http>,
    filesystem: Arc<dyn FileSystem>,
    targets: Vec<Target>,
}

impl CredentialsCheck {
    /// A check over the given transport, filesystem and services to prove.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, filesystem: Arc<dyn FileSystem>, targets: Vec<Target>) -> Self {
        Self {
            http,
            filesystem,
            targets,
        }
    }

    /// Read one service's key from disk, then ask it who it is with that key.
    ///
    /// An absent file or an empty key is a service still completing its first
    /// start, skipped so a later run proves it — not a fault. Beyond that, the
    /// three failures the service can present are kept as three distinct verdicts.
    async fn prove(&self, target: &Target) -> Verdict {
        let Some(service) = target.open(&self.http, self.filesystem.as_ref()).await else {
            return not_started(target);
        };
        match service.identity().await {
            Ok(identity) => Verdict::Pass {
                note: Some(format!("{} {}", identity.name, identity.version)),
            },
            Err(Failure::Unauthorised { .. }) => Verdict::Fail(rejected(target)),
            Err(Failure::Unavailable { .. }) => unreachable(target),
            Err(Failure::Refused { detail, .. }) => unusable(target, &detail),
            Err(Failure::Unsupported { detail, .. }) => unsupported(target, &detail),
        }
    }
}

#[async_trait]
impl Check for CredentialsCheck {
    fn category(&self) -> Category {
        Category::Credentials
    }

    async fn run(&self) -> Vec<Finding> {
        let mut findings = Vec::with_capacity(self.targets.len());
        for target in &self.targets {
            findings.push(Finding {
                check: format!("credentials.{}", target.id),
                category: Category::Credentials,
                title: format!("{} credential", target.name),
                verdict: self.prove(target).await,
            });
        }
        findings
    }
}

/// There is no key to read yet: the ordinary case is a service still completing
/// its first start, which a later run picks up once it has written one.
fn not_started(target: &Target) -> Verdict {
    Verdict::Skipped {
        reason: format!(
            "{}'s API key could not be read yet; a service still starting has not written one, and a later run proves it once it has",
            target.name
        ),
    }
}

/// The service answered and refused the key it wrote: the key on disk and the one
/// the running service expects disagree, which a restart reconciles.
fn rejected(target: &Target) -> Problem {
    Problem::new(
        CREDENTIAL_REJECTED,
        Severity::Error,
        format!("{} refused its own credential", target.name),
        "The key in the service's configuration no longer matches the one the running service expects, usually because the configuration was regenerated after the service last started.",
        Remedy::new("Restart the service so it reloads its configuration, then check again")
            .with_detail("lemonfiber restart"),
    )
}

/// Nothing answered, so the credential is unproven rather than broken — never a
/// pass, and pointed at whether the service is up rather than at the key.
fn unreachable(target: &Target) -> Verdict {
    Verdict::Unverified {
        reason: format!(
            "{} did not answer, so its credential could not be proven",
            target.name
        ),
        remedy: Remedy::new("Check the service is running, then check again")
            .with_detail("lemonfiber ps"),
    }
}

/// The service answered, but not in a way that proves the credential. Its own
/// words are carried through rather than paraphrased into something vaguer.
fn unusable(target: &Target, detail: &str) -> Verdict {
    Verdict::Unverified {
        reason: format!("{} answered unusably: {detail}", target.name),
        remedy: Remedy::new("Check the service's own logs for why it is not answering normally")
            .with_detail("lemonfiber logs"),
    }
}

/// The service answered but does not serve the API version this build speaks, so
/// the credential cannot be proven through it — the service was upgraded past (or
/// stands before) the version lemonfiber knows, which aligning the two resolves.
fn unsupported(target: &Target, detail: &str) -> Verdict {
    Verdict::Unverified {
        reason: format!(
            "{} does not serve the API version this build speaks: {detail}",
            target.name
        ),
        remedy: Remedy::new(
            "Match the service to the version lemonfiber supports, or update lemonfiber, then check again",
        )
        .with_detail("lemonfiber --version"),
    }
}
