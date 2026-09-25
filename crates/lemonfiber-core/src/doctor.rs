//! Checks that prove things rather than assuming them.
//!
//! Every check is independent, bounded by its own timeout, and returns a finding
//! that carries a remedy. Checks are values in a collection rather than
//! branches in a function, so one that hangs or fails cannot take the others
//! with it.
//!
//! "Could not check" is a distinct variant of the finding type rather than a
//! severity value, so it cannot accidentally render as "passed" — which is the
//! failure that would make the whole feature dishonest.
//!
//! An error inside a check surfaces as a check error, never as a finding about
//! the stack.
//!
//! See `.docs/architecture/module-layout.md`.

pub mod acknowledged;
pub mod autostart;
pub mod bindings;
mod bundled;
pub mod contributed;
pub mod credentials;
pub mod environment;
mod examining;
#[cfg(test)]
mod fixtures;
pub mod guides;
pub mod headroom;
pub mod indexer;
pub mod narrowing;
pub mod permissions;
pub mod providers;
pub mod releases;
pub mod storage;
pub mod telling;
pub mod vpn;
pub mod wiring;

use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;

use crate::error::{Problem, Remedy};
use crate::repair::{Attempt, Repair, Writing};

pub use narrowing::Narrowing;

pub use bundled::BUNDLED_CHECKS;
pub use examining::{attributed, examine, overall};

/// The family a check belongs to, so a run can be narrowed to one of them.
///
/// These are the diagnostic categories the product recognises; the checks that
/// fill each one arrive over time, so a category may name more than lemonfiber
/// can yet establish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "DoctorCategory")]
pub enum Category {
    /// Docker present, the daemon reachable, the platform understood.
    Environment,
    /// The data root: reachable, writable, one filesystem, room to grow.
    Storage,
    /// Ports free, bindings matching policy, services reachable.
    Network,
    /// Torrent traffic genuinely leaves through the tunnel.
    Vpn,
    /// Each credential still valid.
    Credentials,
    /// Health, crash loops, version skew, the wiring between services.
    Services,
    /// Provider quota, subscription validity, indexer responsiveness.
    Providers,
    /// Stuck items, repeated import failures, orphaned downloads.
    Queue,
    /// Drift from lemonfiber-managed state, permissions, manifest validity.
    Config,
}

impl Category {
    /// The category as the operator names it to `--only`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Storage => "storage",
            Self::Network => "network",
            Self::Vpn => "vpn",
            Self::Credentials => "credentials",
            Self::Services => "services",
            Self::Providers => "providers",
            Self::Queue => "queue",
            Self::Config => "config",
        }
    }

    /// Every category, in the order an operator meets them.
    ///
    /// One list, because everything that has to enumerate them — parsing a name,
    /// and holding the set a plugin may contribute into against this one — was
    /// otherwise writing the list again. A copy is a place the tenth category does
    /// not arrive, and a comparison between two copies that both forgot it passes.
    #[must_use]
    pub const fn every() -> [Self; 9] {
        [
            Self::Environment,
            Self::Storage,
            Self::Network,
            Self::Vpn,
            Self::Credentials,
            Self::Services,
            Self::Providers,
            Self::Queue,
            Self::Config,
        ]
    }

    /// The category an operator named, when it is one lemonfiber knows.
    ///
    /// An unknown name is `None` rather than a silent empty run, so a surface can
    /// tell the operator they mistyped rather than reporting that nothing was
    /// wrong.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::every()
            .into_iter()
            .find(|category| category.as_str() == name)
    }
}

/// How a single check turned out.
///
/// "Could not check" (`Unverified`) is its own variant rather than a level of
/// severity, so a check that could not run can never be mistaken for one that
/// passed — the dishonesty this whole subsystem exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
#[schemars(rename = "DoctorVerdict")]
pub enum Verdict {
    /// Verified working, with the evidence worth showing.
    Pass {
        /// What was observed, where stating it helps — an address, a port.
        note: Option<String>,
    },
    /// Working, but degraded or risky.
    Warn(#[schemars(schema_with = "crate::error::problem_schema")] Problem),
    /// Not working.
    Fail(#[schemars(schema_with = "crate::error::problem_schema")] Problem),
    /// Could not be established. Never a pass.
    Unverified {
        /// Why it could not be determined.
        reason: String,
        /// What the operator can do to get an answer.
        remedy: Remedy,
    },
    /// A prerequisite was absent, so the check did not apply.
    Skipped {
        /// Why the check did not apply.
        reason: String,
    },
}

/// One thing a check established, and how it turned out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Finding {
    /// A stable identifier for the thing checked, such as `vpn.egress-match`.
    pub check: String,
    /// The family this belongs to.
    pub category: Category,
    /// The one-line summary of what was checked.
    pub title: String,
    /// How it turned out.
    pub verdict: Verdict,
    /// The service this is about, where it is about one.
    ///
    /// Absent for the checks that are about the machine rather than about
    /// something running on it — the environment, the filesystem, the operator's
    /// own choices. Carried so that one service's trouble can be attributed to
    /// the service underneath it rather than counted as one more independent
    /// thing wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// The check whose finding explains this one, where another does.
    ///
    /// Set after the run rather than by the check itself: a check is independent
    /// by construction and cannot see what any other found, which is a property
    /// worth keeping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caused_by: Option<String>,
    /// What the service said for itself, lately.
    ///
    /// Carried on the finding rather than left for the operator to go and fetch,
    /// because the explanation is almost always in it: a check can say a service is
    /// not answering, and only the service can say why. Absent where the finding is
    /// not about a service, where the service is fine, or where the engine would not
    /// say — an empty section would be a promise of evidence that is not there.
    ///
    /// Set after the run, like [`Self::caused_by`], since reading a service's output
    /// is not the check's own business and a check that did it would be doing two
    /// things.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
    /// Whose check this is: one this build ships, or one a named plugin contributed.
    ///
    /// Carried rather than read off the identifier. A contributed check's id is
    /// namespaced with the plugin's, and a reader could decode that from the colon —
    /// but an origin a reader has to decode is one a reader gets wrong, and the day a
    /// bundled id grew a colon every such reader would misattribute it in silence.
    pub origin: crate::origin::Origin,
}

impl Finding {
    /// A finding in a given category. Each check module wraps this with its own
    /// category bound, so the shape of a finding is written once here rather than
    /// re-spelled per module.
    #[must_use]
    pub fn in_category(category: Category, check: &str, title: &str, verdict: Verdict) -> Self {
        Self {
            check: check.to_owned(),
            category,
            title: title.to_owned(),
            verdict,
            service: None,
            caused_by: None,
            said: None,
            origin: crate::origin::Origin::Bundled,
        }
    }

    /// The same finding, said to be about a particular service.
    #[must_use]
    pub fn about(mut self, service: &str) -> Self {
        self.service = Some(service.to_owned());
        self
    }
}

/// What a run's findings amount to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Overall {
    /// Everything that ran passed.
    Healthy,
    /// Warnings, but nothing broken.
    Degraded,
    /// Something is broken.
    Broken,
    /// Something could not be determined, so health is not a settled fact.
    Unknown,
}

/// How long a single check may run before it is abandoned as unverified.
///
/// Generous enough for a container command over a busy daemon, bounded because a
/// wait with no end is indistinguishable from a hang.
const CHECK_BUDGET: Duration = Duration::from_secs(15);

/// How long a check that waits on a filesystem may run.
///
/// Longer than [`CHECK_BUDGET`], because what it is waiting on is not a container
/// command but a disk. A network share reached over a busy link, or an external
/// drive that has spun down, can take tens of seconds to answer its first
/// request — and abandoning it would report hardware that is merely slow as
/// hardware that cannot be read, which is the worse of the two mistakes: an
/// operator sent to diagnose a working disk.
///
/// Thirty seconds rather than something larger, because two promises meet here.
/// A run's wall clock is its **slowest** check rather than the sum of them — they
/// run concurrently — so this is exactly the largest a single check can ask for
/// while a full non-disruptive run still finishes inside the thirty seconds it is
/// meant to. Beyond that, one slow disk would break the promise made about every
/// other run.
pub(crate) const FILESYSTEM_BUDGET: Duration = Duration::from_secs(30);

/// How long a check disturbs what it disturbs, in the words an operator reads before
/// deciding whether to let it.
///
/// Naming what is disturbed is half an answer. An operator weighing a check that stops
/// their transfers is weighing being without them for a while, and "a while" is the part
/// they cannot look up — a second is a shrug and ten minutes is a different decision.
///
/// Said from the bound that enforces it rather than from a number written into a
/// sentence, so the length promised and the length allowed cannot drift apart. Which
/// bound that is differs: a check whose whole run is the burden says its budget, and one
/// that takes something away for part of its run says how long it is away for.
pub(crate) fn disturbing_for(bound: Duration) -> String {
    format!(
        "for no longer than the {} seconds it is bounded to",
        bound.as_secs()
    )
}

/// What a check reports against, for a check that reports against one thing.
///
/// Identity and subject rather than a whole finding, deliberately. A check that could
/// hand back a finding of its own for the moments it did not run could hand back a
/// passing one, and "could not run" must never be sayable as "passed" — so the verdict
/// on those moments stays the run's to decide and only the naming is the check's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reported {
    /// The id its finding carries, and the name it can be asked for by.
    pub check: String,
    /// The service the finding is about, where it is about one.
    pub service: Option<String>,
    /// Whose check it is, so a finding the run writes in its place — one abandoned at
    /// its budget — is attributed as the check's own would have been.
    pub origin: crate::origin::Origin,
}

/// A single diagnostic, run in isolation from every other.
///
/// A check reports what it could not determine as an unverified finding and
/// never returns an error: one check's trouble must not become a finding about
/// the stack, nor stop the others running. What a check needs to do its work it
/// holds, rather than reaching for a shared context, so the checks stay values
/// in a list.
#[async_trait]
pub trait Check: Send + Sync {
    /// The family this check belongs to, so a run can be narrowed to it.
    fn category(&self) -> Category;

    /// How long this check may run before it is abandoned.
    ///
    /// The default suits a check bounded by a container command; a filesystem
    /// check over a slow NAS would want longer.
    fn budget(&self) -> Duration {
        CHECK_BUDGET
    }

    /// Establish the findings empirically.
    async fn run(&self) -> Vec<Finding>;

    /// The one identity this check reports against, where it has exactly one.
    ///
    /// Nothing for a check that reports a finding per account, per indexer or per
    /// service: what those report against is not known until they have run, so there
    /// is no honest single answer to give. A check that has one says so.
    ///
    /// Two things read it, and both are about a check that has not produced a finding
    /// and never will. A run narrowed to a name the family cannot resolve asks each
    /// check whether the name is its own, and a check abandoned at its budget is
    /// reported against this rather than against its family — because a report saying
    /// only `services` about a check that timed out leaves the operator to work out
    /// which of them it was.
    fn reports(&self) -> Option<Reported> {
        None
    }

    /// What this check can put right about what it found, where it can put anything
    /// right at all.
    ///
    /// Nothing, for most of them — a check that can only observe says so by saying
    /// nothing here. A check that can mend returns itself, which is the point of asking
    /// the check rather than a table somewhere else: it already holds the ports it would
    /// need, and the knowledge of how to fix a thing cannot drift away from the code that
    /// detects it if the two are the same object.
    fn mender(&self) -> Option<&dyn Mend> {
        None
    }
}

/// Putting right what a check found.
///
/// Separate from [`Check`] rather than folded into it, because the two are asked at
/// different moments and by different callers: everything runs `run`, and only a run the
/// operator has told to act asks anything here.
///
/// Neither half decides whether to go ahead. What may be offered is
/// [`crate::repair`]'s, purely, and whether this run may act at all is the operator's —
/// a mender that consulted either would be a second place for the answer to be wrong.
#[async_trait]
pub trait Mend: Send + Sync {
    /// What this check could put right, given what it just found.
    ///
    /// Built from the findings rather than from nothing, because a repair states what it
    /// would do and that sentence depends on what is actually wrong.
    fn repairs(&self, found: &[Finding]) -> Vec<Repair>;

    /// Carry one out.
    ///
    /// Reports what happened and no more. Whether the fault is *gone* is not this
    /// method's to say — that takes asking the check again, which the caller does.
    async fn mend(&self, repair: &Repair) -> Attempt;

    /// Whether this may write what it would write.
    ///
    /// Asked before anything is carried out, so that a repair which must not go ahead is
    /// never attempted rather than attempted and reported as having done nothing. Most
    /// menders have nothing to refuse: a repair that touches no configuration cannot write
    /// over an operator's own change, and says so by not answering the question.
    ///
    /// The ones that do touch configuration read the baseline, which records what
    /// lemonfiber wrote and what it adopted from the operator — and those are different
    /// claims about the same field. They also read what the service holds *now*: "lemonfiber
    /// wrote this once" and "lemonfiber's value is what is there" are different claims too,
    /// and only the second makes a field lemonfiber's to write again. That takes asking the
    /// service, which is why this may await.
    async fn may_proceed(&self, _repair: &Repair) -> Writing {
        Writing::Ours
    }

    /// What carrying this out would write to, by the names a declaration uses — a
    /// service id, a settings key, a path within the stack.
    ///
    /// Nothing for a repair that touches nothing anybody could have declared theirs:
    /// restarting a container writes to no file and no setting, and a mender naming
    /// something here would be claiming a write it does not make.
    ///
    /// **Asked by the caller that carries repairs out, never by the mender itself.**
    /// That is the whole of the design rather than a detail of it: a mender says what
    /// it would touch and the decision is taken once, above the handler, in a match
    /// the compiler checks. A gate inside each mender is a gate somebody adds a third
    /// mender beside, and the promise it keeps — that lemonfiber never writes to an
    /// area the operator declared unmanaged — is one a third mender breaks in silence.
    ///
    /// Not `may_proceed`'s business either, for the reason that method's own doc gives
    /// about deciding: this answer needs nothing of a service, is settled before a run
    /// begins, and a mender that folded it in would be reaching a service the operator
    /// asked it to leave alone in order to find out whether to leave it alone.
    fn writes_to(&self, _repair: &Repair) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests;
