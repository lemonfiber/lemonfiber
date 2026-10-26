//! Proving each service still answers to the credential lemonfiber holds.
//!
//! A wrong or stale credential is the quietest failure a stack has: everything
//! reports healthy and searches simply come back empty, weeks apart from the
//! cause. So this does not check the key's shape — it asks each curator who it is
//! through the adapter in front of it, which presents the key lemonfiber minted for
//! it, and reports what it observed. A proven credential names the service and its
//! version; there is no green tick without a fact behind it.
//!
//! The three ways it can go wrong are kept apart, because collapsing them sends
//! the operator after the wrong problem. A service that answers and refuses the
//! key is a fault to fix; one that does not answer at all leaves the credential
//! unproven rather than broken; one that answers unusably is carried through in
//! the service's own words rather than guessed at.
//!
//! The credential itself never reaches the core's side of the contract: the adapter
//! holds it. What is reported is the outcome, never the input.
//!
//! See `.docs/architecture/module-layout.md`.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_contract::capabilities::library::curate;

use super::{Category, Check, Finding, Verdict};
use crate::error::codes::cred::CREDENTIAL_REJECTED;
use crate::error::{Problem, Remedy};
use crate::ports::service::Failure;

/// One curator whose credential is proven: the service that files media, and the adapter
/// in front of it, presenting the key lemonfiber minted for it, where it can be asked.
#[derive(Clone)]
pub struct Target {
    /// The id of the service that files media, such as `sonarr`; names the finding.
    pub id: String,
    /// Its name, such as `Sonarr`; what the operator reads.
    pub name: String,
    /// The kind of video it files, read from the media types it declares; nothing for
    /// one that files video by no resolution, such as music.
    pub kind: Option<crate::ports::media::Kind>,
    /// The adapter it is asked through, or nothing where it cannot be asked.
    pub asked: Option<Arc<dyn curate::Fills>>,
}

/// Proves each curator still answers to the key lemonfiber minted for it.
pub struct CredentialsCheck {
    targets: Vec<Target>,
}

impl CredentialsCheck {
    /// A check over the curators given.
    #[must_use]
    pub const fn new(targets: Vec<Target>) -> Self {
        Self { targets }
    }

    /// Ask one curator who it is, through its adapter.
    ///
    /// One that cannot be asked yet is skipped so a later run proves it, not a fault.
    /// Beyond that, the three failures the service can present are kept as three
    /// distinct verdicts.
    async fn prove(target: &Target) -> Verdict {
        let Some(service) = &target.asked else {
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
        ran(self).await
    }
}

/// Whether every service that needs a key has one that works.
async fn ran(check: &CredentialsCheck) -> Vec<Finding> {
    let mut findings = Vec::with_capacity(check.targets.len());
    for target in &check.targets {
        findings.push(
            Finding::in_category(
                Category::Credentials,
                &format!("credentials.{}", target.id),
                &format!("{} credential", target.name),
                CredentialsCheck::prove(target).await,
            )
            // Said so that a service which cannot answer because the thing underneath
            // it is down reads as one problem rather than as two.
            .about(&target.id),
        );
    }
    findings
}

/// The adapter in front of it cannot be asked yet: the ordinary case is a service still
/// completing its first start, which a later run proves once it answers.
fn not_started(target: &Target) -> Verdict {
    Verdict::Skipped {
        reason: format!(
            "{} could not be asked yet; the adapter in front of it is still starting or has not answered as its contract says, and a later run proves it once it does",
            target.name
        ),
    }
}

/// The service answered and refused the key lemonfiber minted for it: the key the running
/// service reads and the one its adapter presents disagree, which a restart reconciles.
fn rejected(target: &Target) -> Problem {
    Problem::new(
        CREDENTIAL_REJECTED,
        format!("{} refused its own credential", target.name),
        "The key lemonfiber minted for the service no longer matches the one the running service expects, usually because the key was replaced after the service last started.",
        Remedy::new("Restart the service so it reads its key again, then check again")
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
