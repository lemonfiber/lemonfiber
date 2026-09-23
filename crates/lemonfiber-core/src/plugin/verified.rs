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
mod tests {
    use super::{against, standing, Standing, Verification};
    use crate::doctor::{Category, Finding, Verdict};
    use crate::error::{Code, Problem, Remedy, Severity};

    /// A finding under a named check, which is the key the whole rule turns on.
    fn finding(check: &str, verdict: Verdict) -> Finding {
        Finding {
            check: check.to_owned(),
            category: Category::Network,
            title: format!("what {check} establishes"),
            service: None,
            caused_by: None,
            said: None,
            verdict,
        }
    }

    /// A verdict that is raising something, at either height.
    fn raised(failing: bool) -> Verdict {
        let problem = Problem::new(
            Code::new("TEST-1"),
            Severity::Error,
            "It broke",
            "The thing did not happen",
            Remedy::new("Try again"),
        );
        if failing {
            Verdict::Fail(problem)
        } else {
            Verdict::Warn(problem)
        }
    }

    /// A verdict that says nothing could be established.
    fn untold() -> Verdict {
        Verdict::Unverified {
            reason: "nothing answered".to_owned(),
            remedy: Remedy::new("Try again"),
        }
    }

    /// The checks a run names, in the order the doctor produced them.
    fn named(verification: &[super::Changed]) -> Vec<String> {
        verification
            .iter()
            .map(|one| one.now.check.clone())
            .collect()
    }

    /// The whole of what the rule buys: a check the install made worse is named, and
    /// one that was already failing is not — which is the difference between *this
    /// install broke it* and *this machine was already like that*.
    #[test]
    fn a_check_this_install_made_worse_is_named_and_one_already_failing_is_not() {
        let before = vec![
            finding("network.bindings", Verdict::Pass { note: None }),
            finding("providers.usenet", raised(true)),
        ];
        let after = vec![
            finding("network.bindings", raised(true)),
            finding("providers.usenet", raised(true)),
        ];

        let verification = against(&before, &after);
        assert_eq!(named(&verification.broke), vec!["network.bindings"]);
        assert!(verification.unsettled.is_empty());
        assert!(!verification.held(), "a broken check stops the install");
    }

    /// A warning the install turns into a failure is the install making it worse, and
    /// a ladder of two rungs is what catches it. One rung for *raising something* would
    /// call this no change at all.
    #[test]
    fn a_warning_this_install_turned_into_a_failure_is_named() {
        let before = vec![finding("storage.space", raised(false))];
        let after = vec![finding("storage.space", raised(true))];

        let verification = against(&before, &after);
        assert_eq!(named(&verification.broke), vec!["storage.space"]);
    }

    /// A check that goes quiet is not an install to reverse, and neither is a check
    /// that was not raised before and holds now.
    #[test]
    fn a_check_that_stopped_being_raised_is_no_event_at_all() {
        let before = vec![
            finding("storage.space", raised(true)),
            finding("environment.engine", raised(false)),
        ];
        let after = vec![finding("storage.space", Verdict::Pass { note: None })];

        let verification = against(&before, &after);
        assert!(
            verification.broke.is_empty(),
            "an improvement is not a break"
        );
        assert!(verification.unsettled.is_empty());
        assert!(verification.held());
    }

    /// A check nothing raised beforehand and that fails now is this install's doing,
    /// because absent reads as holding — which is the same rule the repair's proof
    /// takes, read backwards.
    #[test]
    fn a_check_that_was_not_raised_before_and_fails_now_counts_against_the_install() {
        let after = vec![finding("network.bindings", raised(true))];

        let verification = against(&[], &after);
        assert_eq!(named(&verification.broke), vec!["network.bindings"]);
    }

    /// Assurance that was there and is not says so, and does not stop the install: the
    /// check is not failing, it is saying it could not tell, and reversing on that
    /// would punish a plugin for a provider that was unreachable for a moment.
    #[test]
    fn a_check_that_could_no_longer_be_told_is_unsettled_rather_than_broken() {
        let before = vec![finding("providers.usenet", Verdict::Pass { note: None })];
        let after = vec![finding("providers.usenet", untold())];

        let verification = against(&before, &after);
        assert!(verification.broke.is_empty());
        assert_eq!(named(&verification.unsettled), vec!["providers.usenet"]);
        assert!(
            verification.held(),
            "what could not be told does not take an install back"
        );
    }

    /// And the mirror: failing now, with nothing beforehand to say whether it was.
    /// Naming the install as the cause would be a guess with a reversal attached.
    #[test]
    fn a_check_that_could_not_be_told_before_and_fails_now_is_unsettled_too() {
        let before = vec![finding("guides.reachable", untold())];
        let after = vec![finding("guides.reachable", raised(true))];

        let verification = against(&before, &after);
        assert!(verification.broke.is_empty());
        assert_eq!(named(&verification.unsettled), vec!["guides.reachable"]);
        assert_eq!(
            verification
                .unsettled
                .first()
                .and_then(|one| one.before.clone()),
            Some(untold()),
            "how it read before is carried, so the reader can see there was nothing to compare"
        );
    }

    /// One that could not be told before and cannot be told now has not changed, and
    /// one that could not be told before and holds now is an improvement. Neither is
    /// worth a line.
    #[test]
    fn what_could_never_be_told_and_what_came_right_are_both_silent() {
        let before = vec![
            finding("guides.reachable", untold()),
            finding("vpn.egress-match", untold()),
        ];
        let after = vec![
            finding("guides.reachable", untold()),
            finding("vpn.egress-match", Verdict::Pass { note: None }),
        ];

        let verification = against(&before, &after);
        assert!(verification.broke.is_empty());
        assert!(verification.unsettled.is_empty());
    }

    /// A check that did not apply to this machine found nothing wrong with it, so it
    /// holds. Reading skipped as anything else would have an install reversed by a
    /// check that declined to run.
    #[test]
    fn a_skipped_check_holds_and_a_run_that_skips_more_is_not_a_break() {
        let before = vec![finding("services.releases", Verdict::Pass { note: None })];
        let after = vec![finding(
            "services.releases",
            Verdict::Skipped {
                reason: "not asked for".to_owned(),
            },
        )];

        assert_eq!(
            after.first().and_then(|one| standing(&one.verdict)),
            Some(Standing::Held)
        );
        assert!(against(&before, &after).held());
    }

    /// Every verdict the doctor can reach has a rung or says it has none, and the one
    /// without is the one that must not be comparable to the others.
    #[test]
    fn every_verdict_says_where_it_stands_or_that_it_cannot() {
        assert_eq!(
            standing(&Verdict::Pass { note: None }),
            Some(Standing::Held)
        );
        assert_eq!(standing(&raised(false)), Some(Standing::Raised));
        assert_eq!(standing(&raised(true)), Some(Standing::Broken));
        assert_eq!(standing(&untold()), None);
        assert!(Standing::Held < Standing::Raised && Standing::Raised < Standing::Broken);
    }

    /// Nothing changed is what an install needs, and it is the common answer.
    #[test]
    fn a_run_that_changed_nothing_holds() {
        let verification = Verification {
            broke: Vec::new(),
            unsettled: Vec::new(),
        };
        assert!(verification.held());
    }
}
