//! What a repairing run writes down about a fault, and what that record decides later.
//!
//! Apart from the running of it because the two answer questions on different timescales.
//! Carrying a repair out is about now: it either worked or it did not, and the second look
//! settles that before the run ends. This is about the runs either side — how often a
//! fault has been seen, how often a fix for it was tried and left it standing, and whether
//! the operator has already said no. None of it can be read off the run that is happening,
//! and all of it decides what the next run offers.

use crate::app::repairs::Entry;
use crate::app::{conditions, repairs, Ctx};
use crate::condition::{Condition, Fault};
use crate::doctor::{Finding, Verdict};
use crate::repair::{self, Outcome, Repair};

use super::Beyond;

/// Fold what this run found into the store, and answer with it.
///
/// The same folding the dashboard does for services, for the same reason: how long a fault
/// has stood, whether it flaps, whether a fix was declined and how often one has failed are
/// all comparisons against previous runs, and none of them can be made by a store that has
/// never heard of the check.
pub(super) fn remembered(ctx: &Ctx, found: &[Finding]) -> crate::condition::Conditions {
    let mut conditions = conditions::load(ctx);
    let now = ctx.stamp();
    for finding in found {
        conditions.observe(&finding.check, wrong(finding).as_ref(), &now);
    }
    // Written down only by a run that is really happening. What this file holds is how
    // often a fault has been seen and how often a fix for it was tried and left it
    // standing, which is how the offer decides what is worth offering again — and a
    // rehearsal that recorded a sighting would move that count without anybody having
    // asked it to. The reading above still happens, because the report a rehearsal
    // gives is built from it.
    if !ctx.dry_run {
        conditions::save(ctx, &conditions);
    }
    conditions
}

/// What a finding is remembered as, where it says something is wrong.
///
/// A pass says nothing is wrong and a skip says there was nothing to look at, so neither
/// raises anything. Unverified is the careful one: it means the check could not be
/// established, which is not the same as finding it broken — claiming a fault from it would
/// have lemonfiber remember trouble it never actually saw.
pub(super) fn wrong(finding: &Finding) -> Option<Fault> {
    let problem = match &finding.verdict {
        Verdict::Warn(problem) | Verdict::Fail(problem) => problem,
        Verdict::Pass { .. } | Verdict::Skipped { .. } | Verdict::Unverified { .. } => return None,
    };
    Some(Fault::new(
        problem.code.as_str(),
        problem.severity,
        &problem.summary,
        &problem.meaning,
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
pub(super) fn beyond(conditions: &[&Condition], proposals: &[Repair]) -> Vec<Beyond> {
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

/// Remember that the operator said no, so it stops being offered until the fault has been
/// away and genuinely come back.
pub(super) fn declined(ctx: &Ctx, repair: &Repair) {
    let mut conditions = conditions::load(ctx);
    conditions.decline(&repair.check);
    conditions::save(ctx, &conditions);
}

/// Record what was done and how it turned out.
///
/// Both halves, always. The count of attempts that left the fault standing, so a repair
/// that is not working stops being offered — and the history, including the attempts that
/// changed nothing, which are the entries somebody reads when the same repair keeps
/// failing to hold a fault down.
pub(super) fn recorded(ctx: &Ctx, repair: &Repair, outcome: &Outcome) {
    let mut conditions = conditions::load(ctx);
    match outcome {
        Outcome::Fixed => conditions.mended(&repair.check),
        // Spent only where the repair ran and the fault is demonstrably still there. One
        // that stopped, was declined, or could not be proved either way has told us nothing
        // about whether lemonfiber is wrong about the cause, which is what the count means.
        Outcome::FixFailed => conditions.attempted(&repair.check),
        Outcome::Stopped { .. }
        | Outcome::Declined
        | Outcome::WouldOverwrite
        | Outcome::Unmanaged => {}
    }
    conditions::save(ctx, &conditions);

    let mut history = repairs::load(ctx);
    history.record(Entry {
        at: ctx.stamp(),
        check: repair.check.clone(),
        did: repair.does.clone(),
        outcome: outcome.clone(),
    });
    repairs::save(ctx, &history);
}

#[cfg(test)]
mod tests {
    use super::wrong;
    use crate::doctor::{Category, Finding, Verdict};
    use crate::error::{Code, Problem, Remedy, Severity};

    const CODE: Code = Code::new("storage.full");

    /// A finding whose verdict carries the full four parts a problem is made of.
    fn found(verdict: Verdict) -> Finding {
        Finding {
            check: "storage.space".to_owned(),
            category: Category::Storage,
            title: "room on the volume".to_owned(),
            verdict,
            service: None,
            caused_by: None,
            said: None,
            origin: crate::origin::Origin::Bundled,
        }
    }

    /// The problem a full volume amounts to.
    fn full() -> Problem {
        Problem::new(
            CODE,
            Severity::Error,
            "the volume has no room left",
            "nothing can be written, so imports will fail where they stand",
            Remedy::new("delete something, or move the library"),
        )
    }

    #[test]
    fn what_a_finding_meant_survives_into_the_condition_it_raises() {
        // The verdict states what happened, what it means and what to do; the
        // condition is what every later surface reads. Keeping two of the three
        // here is how a doctor screen and a dashboard come to disagree about the
        // same problem.
        let remembered = wrong(&found(Verdict::Fail(full())));
        let parts = remembered.map(|fault| (fault.summary, fault.meaning));
        assert_eq!(
            parts,
            Some((
                "the volume has no room left".to_owned(),
                "nothing can be written, so imports will fail where they stand".to_owned(),
            ))
        );
    }

    #[test]
    fn a_check_that_found_nothing_wrong_remembers_nothing() {
        // A pass and a skip say nothing is wrong and nothing was looked at, and an
        // unverified check found no fault — only that it could not say.
        for verdict in [
            Verdict::Pass { note: None },
            Verdict::Skipped {
                reason: "no volume configured".to_owned(),
            },
            Verdict::Unverified {
                reason: "the volume could not be read".to_owned(),
                remedy: Remedy::new("check the path exists"),
            },
        ] {
            assert!(wrong(&found(verdict)).is_none());
        }
    }
}
