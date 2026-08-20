//! Carrying out the repairs a diagnosis offered, and proving they worked.
//!
//! The order is the whole of the design: look, offer, confirm, act, then look again — and
//! only the second look decides what is reported. A repair whose command returned an exit
//! code of zero has proved nothing; what proves it is the check that raised the finding
//! failing to raise it again.
//!
//! Nothing here decides what may be offered. That is [`crate::repair`]'s, purely: what has
//! been declined, what has been tried too often and which findings share a cause are all
//! settled before this module sees them. What is here is the running of it.

use crate::condition::Fault;
use crate::config::paths::{Paths, JOURNAL};
use crate::doctor::{Check, Finding, Mend, Verdict};
use crate::error::{Diagnose as _, Problem, Remedy};
use crate::journal::Undo;
use crate::repair::{self, Attempt, Outcome, Repair, Stance};

use super::repairs::Entry;
use super::Ctx;

/// Whoever decides, for this run, whether a repair goes ahead.
///
/// A trait rather than a closure: a surface that builds one to capture what it asks with
/// builds a function of its own, and the sequence here would be copied around it. What
/// asks is the surface's business either way — this only needs the answer.
pub trait Confirm {
    /// Whether this repair may be carried out.
    fn agreed(&self, repair: &Repair) -> bool;
}

/// One repair, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Mended {
    /// What was proposed.
    pub repair: Repair,
    /// How it turned out, once the check was asked again.
    pub outcome: Outcome,
}

/// A repair that has run out of chances, and where to go instead.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Beyond {
    /// The check whose fault has outlasted every attempt at it.
    pub check: String,
    /// What to do about it now that lemonfiber has stopped offering to.
    pub remedy: Remedy,
}

/// What a repairing run offered, and what it did.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct Report {
    /// What could be put right, whether or not it was.
    pub offered: Vec<Repair>,
    /// What was carried out, in the order it was.
    pub mended: Vec<Mended>,
    /// What has been tried too often to keep offering.
    ///
    /// Said rather than passed over. A repair that quietly stopped being offered leaves
    /// the operator watching a fault nobody mentions any more, which is worse than being
    /// told plainly that this is past what lemonfiber can work out.
    pub beyond: Vec<Beyond>,
    /// Whether this run was allowed to act at all.
    pub acted: bool,
}

/// Offer what can be put right, carry out what is confirmed, and prove each one.
///
/// `confirm` is asked once per repair, and only where the stance says to ask — an
/// unattended run was confirmed by the flag that made it unattended. It is a closure for
/// the reason [`super::forwarding::push`] takes one: the deciding belongs to whichever
/// surface is running this, and the sequence belongs here. Synchronous, because deciding
/// is: a surface that must wait on something to answer can wait before calling this.
///
/// Taken as a trait object rather than by type, so there is one of this function however
/// many surfaces call it — a generic would be copied per call site, and the copy nothing
/// drives is a copy the coverage gate counts and no test can reach.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read, which is the one thing every
/// check needs before any of this can begin.
pub async fn mend(
    ctx: &Ctx,
    stance: Stance,
    disruptive: bool,
    confirm: &dyn Confirm,
) -> Result<Report, Box<Problem>> {
    let checks = super::engine::assembled(ctx, disruptive).await?;
    // A second set, assembled without the disruptive ones, for proving the work. Built
    // here rather than per repair: nine checks constructed once are nine constructed once,
    // however many faults this run puts right.
    let again = super::engine::assembled(ctx, false).await?;
    Ok(mending(ctx, &checks, &again, stance, confirm).await)
}

/// The same errand, over checks somebody else assembled.
///
/// Apart from [`mend`] so that what a repair *does* can be driven against checks written
/// for the purpose — a runner reachable only through nine real checks and a live stack is
/// one nobody writes a test for, which is how two defects got into this module's first
/// draft.
///
/// `again` is what proves the work, and is a second set rather than the same one: a check
/// holds what it read when it was built, so asking the very instances that found the fault
/// would compare a repair with the reading it was meant to change.
pub async fn mending(
    ctx: &Ctx,
    checks: &[Box<dyn Check>],
    again: &[Box<dyn Check>],
    stance: Stance,
    confirm: &dyn Confirm,
) -> Report {
    let found = looked(ctx, checks).await;

    // Remembered before anything is offered. Whether a fix was declined, and how often one
    // has been tried and left the fault standing, are questions about a check across runs —
    // and the store is where lemonfiber keeps that. A run that consulted it without first
    // telling it what this run found would be asking about faults it had never mentioned.
    let conditions = remembered(ctx, &found);
    let proposals = proposed(checks, &found);
    let all: Vec<Repair> = proposals.iter().map(|(_, repair)| repair.clone()).collect();
    let offered = repair::offered(&all, &conditions.all());

    let mut report = Report {
        offered: offered.clone(),
        mended: Vec::new(),
        beyond: beyond(&conditions.all(), &all),
        acted: stance.may_act(),
    };
    if !stance.may_act() {
        return report;
    }

    // Each survivor keeps the mender that offered it. Looking one up again by check name
    // would need a branch for the case where it is not found — which cannot happen, and so
    // could not be tested, and would have to claim something untrue if it ever ran.
    for (mender, repair) in proposals
        .into_iter()
        .filter(|(_, proposal)| offered.iter().any(|kept| kept.check == proposal.check))
        .filter_map(|(at, proposal)| checks.get(at)?.mender().map(|found| (found, proposal)))
    {
        if stance.asks() && !confirm.agreed(&repair) {
            declined(ctx, &repair);
            report.mended.push(Mended {
                repair,
                outcome: Outcome::Declined,
            });
            continue;
        }
        // Asked before anything is carried out: a repair that must not go ahead is never
        // attempted, rather than attempted and reported as having changed nothing.
        let outcome = if mender.may_proceed(&repair).await.allowed() {
            carried(ctx, mender, again, &repair).await
        } else {
            Outcome::WouldOverwrite
        };
        recorded(ctx, &repair, &outcome);
        report.mended.push(Mended { repair, outcome });
    }
    report
}

/// Put back what the last repair changed, and say what went back.
///
/// That repair and no other. The journal it reads is shared with seeding and the first-run
/// wizard, and somebody undoing the thing they just watched happen has not asked for the
/// wiring underneath it to come apart.
///
/// Two reversals, in order: the changes that live inside a service go back through that
/// service, and what is left — settings, directories — goes back on the host. In that
/// order because the service is the part that can be unreachable, and an operator whose
/// Sonarr is down should still get their environment file back.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read, where a setting or directory
/// could not be put back, or where a change needed a service that would not answer — the
/// last of which names every such change together rather than one at a time.
pub async fn retract(ctx: &Ctx, paths: &Paths) -> Result<Vec<Undo>, Box<Problem>> {
    let undos = repair::undoing(super::recover::journal_at(&paths.journal()).changes());
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let project = super::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let (left, unreached) =
        super::recover::reconfigured(ctx, &undos, &manifest.services, project.as_deref()).await;
    super::recover::undo(&left, &paths.env_file(), unreached)?;
    Ok(undos)
}

/// Fold what this run found into the store, and answer with it.
///
/// The same folding the dashboard does for services, for the same reason: how long a fault
/// has stood, whether it flaps, whether a fix was declined and how often one has failed are
/// all comparisons against previous runs, and none of them can be made by a store that has
/// never heard of the check.
fn remembered(ctx: &Ctx, found: &[Finding]) -> crate::condition::Conditions {
    let mut conditions = super::conditions::load(ctx);
    let now = ctx.stamp();
    for finding in found {
        conditions.observe(&finding.check, wrong(finding).as_ref(), &now);
    }
    super::conditions::save(ctx, &conditions);
    conditions
}

/// What a finding is remembered as, where it says something is wrong.
///
/// A pass says nothing is wrong and a skip says there was nothing to look at, so neither
/// raises anything. Unverified is the careful one: it means the check could not be
/// established, which is not the same as finding it broken — claiming a fault from it would
/// have lemonfiber remember trouble it never actually saw.
fn wrong(finding: &Finding) -> Option<Fault> {
    let problem = match &finding.verdict {
        Verdict::Warn(problem) | Verdict::Fail(problem) => problem,
        Verdict::Pass { .. } | Verdict::Skipped { .. } | Verdict::Unverified { .. } => return None,
    };
    Some(Fault::new(
        problem.code.as_str(),
        problem.severity,
        &problem.summary,
        problem
            .remedies
            .first()
            .map_or("", |remedy| remedy.action.as_str()),
    ))
}

/// The faults a repair could answer and has stopped being offered for.
///
/// Only where something could still have been offered: a check nothing can mend was never
/// going to be repaired, and telling its operator that repairs have been exhausted would
/// be describing something that never happened.
fn beyond(conditions: &[&crate::condition::Condition], proposals: &[Repair]) -> Vec<Beyond> {
    conditions
        .iter()
        .filter(|condition| condition.is_raised())
        .filter(|condition| repair::exhausted(condition))
        .filter(|condition| {
            proposals
                .iter()
                .any(|repair| repair.check == condition.check)
        })
        .map(|condition| Beyond {
            check: condition.check.clone(),
            remedy: repair::escalation(condition),
        })
        .collect()
}

/// Every repair the checks that can mend would offer for what was just found, each kept
/// beside the check that offered it — because a repair names its finding and there is no
/// way back from a finding to the thing that raised it.
fn proposed(checks: &[Box<dyn Check>], found: &[Finding]) -> Vec<(usize, Repair)> {
    checks
        .iter()
        .enumerate()
        .filter_map(|(at, check)| check.mender().map(|mender| (at, mender)))
        .flat_map(|(at, mender)| {
            mender
                .repairs(found)
                .into_iter()
                .map(move |repair| (at, repair))
        })
        .collect()
}

/// What the checks find, through the very path a diagnosis takes.
///
/// Shared rather than repeated: a check the operator has already answered must not read as
/// freshly failing because a repair asked again, and a second copy of that rule is a
/// second place for it to be forgotten.
async fn looked(ctx: &Ctx, checks: &[Box<dyn Check>]) -> Vec<Finding> {
    super::engine::examined(ctx, checks, None).await.findings
}

/// Carry one out, then ask again whether the fault is gone.
///
/// The checks are **assembled afresh** for the proof. A check holds what it read when it was
/// built — the download client's listening port among it — so asking the same instances
/// again would compare the repair's work against the very reading it was meant to change,
/// and report every success as a failure.
///
/// Assembled without the disruptive ones, too. Proving a repair worked is no reason to drop
/// the default route or run a live indexer search again, once per repair.
async fn carried(
    ctx: &Ctx,
    mender: &dyn Mend,
    again: &[Box<dyn Check>],
    repair: &Repair,
) -> Outcome {
    let attempt = mender.mend(repair).await;
    // Recorded before the proof, and before anything else can go wrong. What a repair
    // changed is the operator's way back, and a way back that depends on the rest of the
    // run going well is one they find missing exactly when they need it.
    if let Some(journal) = super::targets::beside_env(ctx, JOURNAL) {
        super::recover::journalled(&journal, attempt.changes());
    }
    if matches!(attempt, Attempt::Stopped { .. }) {
        // Nothing changed, or something changed half way. Either way the state it was left
        // in is what the operator needs, and asking the checks again would only rename it.
        return Outcome::of(attempt, false);
    }
    judged(attempt, prove(ctx, again, &repair.check).await)
}

/// Ask again whether the fault is gone.
///
/// Over checks assembled for the purpose rather than the ones that found it: a check holds
/// what it read when it was built, so proving a repair against those very instances would
/// compare its work with the reading it was meant to change — and report every success as
/// a failure.
async fn prove(ctx: &Ctx, again: &[Box<dyn Check>], check: &str) -> Option<bool> {
    proved(&looked(ctx, again).await, check)
}

/// What an attempt and the proof of it amount to together.
///
/// Apart from [`carried`] so that all three answers can be asked of it directly: the one
/// inside a function that assembles nine checks and runs them is an answer no test reaches,
/// and the repo keeps applicable code coverable rather than arguing about it afterwards.
fn judged(attempt: Attempt, proof: Option<bool>) -> Outcome {
    match proof {
        Some(settled) => Outcome::of(attempt, settled),
        // "I could not tell" is not "it is still broken", and reporting it as such would
        // spend one of the few attempts a repair is given on nothing at all.
        None => Outcome::Stopped {
            leaving: "the repair ran, and the check could not be established afterwards".to_owned(),
        },
    }
}

/// Whether the named check now passes, or nothing where it could not be established.
///
/// Absent from the second look counts as passing: a finding no longer raised is a fault no
/// longer there, which is exactly what a repair is for.
fn proved(found: &[Finding], check: &str) -> Option<bool> {
    let Some(finding) = found.iter().find(|finding| finding.check == check) else {
        return Some(true);
    };
    match finding.verdict {
        Verdict::Pass { .. } | Verdict::Skipped { .. } => Some(true),
        Verdict::Warn(_) | Verdict::Fail(_) => Some(false),
        Verdict::Unverified { .. } => None,
    }
}

/// Remember that the operator said no, so it stops being offered until the fault has been
/// away and genuinely come back.
fn declined(ctx: &Ctx, repair: &Repair) {
    let mut conditions = super::conditions::load(ctx);
    conditions.decline(&repair.check);
    super::conditions::save(ctx, &conditions);
}

/// Record what was done and how it turned out.
///
/// Both halves, always. The count of attempts that left the fault standing, so a repair
/// that is not working stops being offered — and the history, including the attempts that
/// changed nothing, which are the entries somebody reads when the same repair keeps
/// failing to hold a fault down.
fn recorded(ctx: &Ctx, repair: &Repair, outcome: &Outcome) {
    let mut conditions = super::conditions::load(ctx);
    match outcome {
        Outcome::Fixed => conditions.mended(&repair.check),
        // Spent only where the repair ran and the fault is demonstrably still there. One
        // that stopped, was declined, or could not be proved either way has told us nothing
        // about whether lemonfiber is wrong about the cause, which is what the count means.
        Outcome::FixFailed => conditions.attempted(&repair.check),
        Outcome::Stopped { .. } | Outcome::Declined | Outcome::WouldOverwrite => {}
    }
    super::conditions::save(ctx, &conditions);

    let mut history = super::repairs::load(ctx);
    history.record(Entry {
        at: ctx.stamp(),
        check: repair.check.clone(),
        did: repair.does.clone(),
        outcome: outcome.clone(),
    });
    super::repairs::save(ctx, &history);
}

#[cfg(test)]
mod tests {
    use super::{beyond, proved, wrong, Beyond};
    use crate::app::fixtures::ctx_at;
    use crate::condition::{Conditions, Fault};
    use crate::doctor::{Category, Finding, Verdict};
    use crate::error::{Code, Problem, Remedy, Severity};
    use crate::repair::{Attempt, Outcome, Repair, ATTEMPTS};

    fn problem() -> Problem {
        Problem::new(
            Code::new("VPN-7"),
            Severity::Warning,
            "it is on the wrong port",
            "peers cannot reach it",
            Remedy::new("move it"),
        )
    }

    fn finding(check: &str, verdict: Verdict) -> Finding {
        Finding::in_category(Category::Vpn, check, "the forwarded port", verdict)
    }

    fn repair(check: &str) -> Repair {
        Repair {
            check: check.to_owned(),
            does: "move the client".to_owned(),
            effects: Vec::new(),
            reversible: false,
        }
    }

    /// What a run found has to reach the store before anything is offered, or every
    /// question asked of it — declined? tried too often? — is asked about a check it has
    /// never heard of, and answered no.
    #[test]
    fn what_a_run_found_is_remembered_before_anything_is_offered() {
        let ctx = ctx_at("repair-remembers");
        let found = vec![
            finding("vpn.port-forward-client", Verdict::Warn(problem())),
            finding("vpn.egress", Verdict::Pass { note: None }),
        ];

        let conditions = super::remembered(&ctx, &found);

        assert!(conditions
            .get("vpn.port-forward-client")
            .is_some_and(crate::condition::Condition::is_raised));
        // A check that has never been wrong gets no entry at all. The store remembers
        // faults, and inventing a cleared condition for something that never failed would
        // fill it with things that never happened — which is also why a repair asks it
        // about the checks that did fail and no others.
        assert!(conditions.get("vpn.egress").is_none());

        // And it survives to the next run, which is the whole point of a store.
        assert!(super::super::conditions::load(&ctx)
            .get("vpn.port-forward-client")
            .is_some());
    }

    /// A pass says nothing is wrong and a skip says there was nothing to look at. An
    /// unverified check is the careful one: it could not be established, which is not the
    /// same as finding it broken.
    #[test]
    fn only_a_finding_that_says_something_is_wrong_is_remembered_as_a_fault() {
        assert!(wrong(&finding("a", Verdict::Warn(problem()))).is_some());
        assert!(wrong(&finding("a", Verdict::Fail(problem()))).is_some());
        assert!(wrong(&finding("a", Verdict::Pass { note: None })).is_none());
        assert!(wrong(&finding(
            "a",
            Verdict::Skipped {
                reason: "nothing to look at".to_owned()
            }
        ))
        .is_none());
        assert!(wrong(&finding(
            "a",
            Verdict::Unverified {
                reason: "could not be established".to_owned(),
                remedy: Remedy::new("try again"),
            }
        ))
        .is_none());
    }

    /// All three answers the proof can give, and what each makes of an attempt that ran.
    #[test]
    fn an_attempt_and_its_proof_together_say_what_happened() {
        use super::judged;

        assert_eq!(judged(Attempt::carried(), Some(true)), Outcome::Fixed);
        assert_eq!(judged(Attempt::carried(), Some(false)), Outcome::FixFailed);
        // Could not be established afterwards: neither fixed nor demonstrably still
        // broken, and reported as what it is rather than as the worse of the two.
        assert!(matches!(
            judged(Attempt::carried(), None),
            Outcome::Stopped { .. }
        ));
        // One that stopped is not judged by the proof at all — what it left behind is
        // what the operator needs, whatever the checks say now.
        assert_eq!(
            judged(
                Attempt::Stopped {
                    leaving: "half of it".to_owned()
                },
                Some(true)
            ),
            Outcome::Stopped {
                leaving: "half of it".to_owned()
            }
        );
    }

    /// The question a repair is judged by. Absent means the fault is gone; unverified means
    /// nobody can say, which must not be mistaken for either answer.
    #[test]
    fn whether_a_check_now_passes_has_three_answers() {
        let check = "vpn.port-forward-client";
        assert_eq!(proved(&[], check), Some(true));
        assert_eq!(
            proved(&[finding(check, Verdict::Pass { note: None })], check),
            Some(true)
        );
        assert_eq!(
            proved(&[finding(check, Verdict::Warn(problem()))], check),
            Some(false)
        );
        assert_eq!(
            proved(
                &[finding(
                    check,
                    Verdict::Unverified {
                        reason: "could not be established".to_owned(),
                        remedy: Remedy::new("try again"),
                    }
                )],
                check
            ),
            None
        );
    }

    /// Said only where something could still have been offered, and only while the fault is
    /// actually standing — clearing a condition does not reset its attempts, so a fault that
    /// went away would otherwise still be announced as beyond repair.
    #[test]
    fn only_a_standing_fault_with_a_repair_is_reported_as_beyond_one() {
        let mut conditions = Conditions::new();
        let fault = Fault::new("vpn.port", Severity::Warning, "wrong port", "move it");
        conditions.observe("vpn.port-forward-client", Some(&fault), "1000");
        for _ in 0..ATTEMPTS {
            conditions.attempted("vpn.port-forward-client");
        }

        let named = |beyond: Vec<Beyond>| -> Vec<String> {
            beyond.into_iter().map(|one| one.check).collect()
        };

        assert_eq!(
            named(beyond(
                &conditions.all(),
                &[repair("vpn.port-forward-client")]
            )),
            vec!["vpn.port-forward-client".to_owned()]
        );

        // Nothing could have repaired it, so nothing has run out of ways to.
        assert!(named(beyond(&conditions.all(), &[])).is_empty());

        // And once it clears, it is not something that outlasted its repairs any more.
        conditions.observe("vpn.port-forward-client", None, "2000");
        assert!(named(beyond(
            &conditions.all(),
            &[repair("vpn.port-forward-client")]
        ))
        .is_empty());
    }
}
