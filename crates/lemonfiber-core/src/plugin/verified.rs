//! What the stack's own checks say about an install, held against what they said
//! before it.
//!
//! **A plugin's proofs cannot see this and that is why it exists.** A proof asks the
//! plugin's own service whether it does what the plugin said it does, and a plugin
//! that answers perfectly may still have taken a port another service was listening
//! on, filled a disk, or put a container on the wrong network. The stack's own
//! diagnosis is the thing that notices, and it is already written.
//!
//! **A differential rather than a verdict, because a machine that was already broken
//! is not this install's doing.** An operator with a failing indexer key installs a
//! plugin; a run that demanded a clean bill of health would refuse the install and
//! name the key, which is true, unhelpful and not what was asked. So the checks are
//! read before anything is written and read again afterwards, and only a check that
//! *changed for the worse* counts against the install.
//!
//! Nothing here touches a disk or a service. The two readings are somebody else's
//! work; this is the rule that reads them, which is what lets every case of it be put
//! in front of a test without a stack under it.

use serde::Serialize;

use crate::doctor::{Finding, Verdict};

/// Where one check stands, on the one ladder a before and an after can be held
/// against.
///
/// Ordered, because *worse* is the whole question and an ordering is how it is
/// asked once rather than in a table of pairs. A warning and a failure are both the
/// check raising something, and they are two rungs rather than one so that a warning
/// an install turns into a failure is caught.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Standing {
    /// The check is satisfied, or had nothing to say on this machine.
    Held,
    /// The check is raising something short of a failure.
    Raised,
    /// The check is failing.
    Broken,
}

/// One check that stands differently after an install than it did before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginChangedCheck")]
pub struct Changed {
    /// The check as it reads now, with everything the diagnosis says about it.
    ///
    /// The finding itself rather than a summary of it, because what an operator does
    /// next is read the remedy, the service's own output and what else is causing it
    /// — and a second, thinner shape of the same fact is where those stop arriving.
    pub now: Finding,
    /// How the same check read before the install, or nothing where it was not
    /// raised at all.
    ///
    /// Absent means the check produced no finding beforehand, which is read as it
    /// holding: a finding no longer raised is a fault no longer there, and the same
    /// rule read backwards is that one not yet raised was not yet a fault.
    pub before: Option<Verdict>,
}

/// What the stack's own checks made of an install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginVerification")]
pub struct Verification {
    /// Every check this install made worse.
    ///
    /// Empty is the answer an install needs, and it is the common one. What is here
    /// is what takes the install back.
    pub broke: Vec<Changed>,
    /// Every check nothing could be concluded about across the two readings.
    ///
    /// Reported and never acted on. *I could not tell* is not *it is still broken*,
    /// and an install reversed because a check could not reach a provider it also
    /// could not reach an hour ago would be punishing a plugin for the weather. It is
    /// said out loud rather than dropped, because the assurance an operator thought
    /// they had is the thing that went.
    pub unsettled: Vec<Changed>,
}

impl Verification {
    /// Whether anything here stops the install.
    ///
    /// The one question a caller asks, so that *what reverses an install* is decided
    /// here beside the rule that fills the lists rather than at whichever caller
    /// happens to read them.
    #[must_use]
    pub fn held(&self) -> bool {
        self.broke.is_empty()
    }
}

/// What one reading of the checks says against another.
///
/// Walked over the second reading rather than the first, which is what makes a check
/// that has *stopped* being raised a non-event: a finding no longer there is a fault
/// no longer there, and an install that quietened something is not an install to
/// reverse.
///
/// Keyed on the check's own identity, which is the identifier a narrowed run is asked
/// for by and the one thing about a finding that is promised to be the same across two
/// runs. A second key kept beside it would be a second place for the two to disagree.
#[must_use]
pub fn against(before: &[Finding], after: &[Finding]) -> Verification {
    let mut verification = Verification {
        broke: Vec::new(),
        unsettled: Vec::new(),
    };
    for finding in after {
        let earlier = before.iter().find(|one| one.check == finding.check);
        let was = earlier.map_or(Some(Standing::Held), |one| standing(&one.verdict));
        let changed = || Changed {
            now: finding.clone(),
            before: earlier.map(|one| one.verdict.clone()),
        };
        match (was, standing(&finding.verdict)) {
            (Some(was), Some(now)) if now > was => verification.broke.push(changed()),
            // Assurance that was there and is not. The check is not failing — it is
            // saying it could not tell — so it is not the install's fault as far as
            // anything here can establish, and saying so is the honest answer.
            (Some(Standing::Held), None) => verification.unsettled.push(changed()),
            // And the mirror: failing now, with nothing to say whether it was failing
            // before. Naming the install as the cause would be a guess with a reversal
            // attached to it.
            (None, Some(now)) if now > Standing::Held => verification.unsettled.push(changed()),
            _ => (),
        }
    }
    verification
}

/// Where a verdict puts its check on the ladder, or nothing where it says it could
/// not tell.
///
/// Skipped counts as held, and deliberately: a check that did not apply to this
/// machine found nothing wrong with it. Unverified is the one that answers with
/// nothing, because *I could not run this* is a fact about the run rather than about
/// the stack, and putting it on the ladder anywhere would make it comparable to
/// something it is not.
fn standing(verdict: &Verdict) -> Option<Standing> {
    match verdict {
        Verdict::Pass { .. } | Verdict::Skipped { .. } => Some(Standing::Held),
        Verdict::Warn(_) => Some(Standing::Raised),
        Verdict::Fail(_) => Some(Standing::Broken),
        Verdict::Unverified { .. } => None,
    }
}

#[cfg(test)]
mod tests;
