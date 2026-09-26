//! Carrying one repair out, and asking again whether the fault is gone.
//!
//! The second look, which is the only thing here that rules on anything. What a mender
//! reported and what exit code a command returned are evidence this collects and does not
//! decide from; the module above says why, and this is where that has to hold.
//!
//! Two things make it easy to get wrong, and each has a function of its own so a test can
//! ask it directly: proving a repair against the very checks that found the fault, and
//! letting "could not tell" count as "still broken".

use crate::app::Ctx;
use crate::config::paths::JOURNAL;
use crate::doctor::{Check, Finding, Mend, Verdict};
use crate::repair::{Attempt, Outcome, Repair};

/// What the checks find, through the very path a diagnosis takes.
///
/// Shared rather than repeated: a check the operator has already answered must not read as
/// freshly failing because a repair asked again, and a second copy of that rule is a
/// second place for it to be forgotten.
pub(super) async fn looked(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    checks: &[Box<dyn Check>],
) -> Vec<Finding> {
    crate::app::engine::examined(ctx, services, checks, &crate::doctor::Narrowing::Suite)
        .await
        .findings
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
pub(super) async fn carried(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    mender: &dyn Mend,
    again: &[Box<dyn Check>],
    repair: &Repair,
) -> Outcome {
    let attempt = mender.mend(repair).await;
    // Recorded before the proof, and before anything else can go wrong. What a repair
    // changed is the operator's way back, and a way back that depends on the rest of the
    // run going well is one they find missing exactly when they need it.
    //
    // A record that cannot be written leaves the repair stopped rather than judged: what
    // it changed stands and cannot be put back, which is the state an operator has to be
    // told they are in before anything asks whether the fault went.
    if let Some(stopped) = unrecorded(ctx, &attempt) {
        return stopped;
    }
    if matches!(attempt, Attempt::Stopped { .. }) {
        // Nothing changed, or something changed half way. Either way the state it was left
        // in is what the operator needs, and asking the checks again would only rename it.
        return Outcome::of(attempt, false);
    }
    judged(attempt, prove(ctx, services, again, &repair.check).await)
}

/// The repair stopped, where what it changed could not be recorded; nothing where it
/// was, or where there is nowhere to record it.
///
/// Apart from [`carried`] because it decides, and a decision inside an async body is
/// one the coverage report sums across the body's states rather than reading as lines.
fn unrecorded(ctx: &Ctx, attempt: &Attempt) -> Option<Outcome> {
    let journal = crate::app::targets::beside_env(ctx, JOURNAL)?;
    let failure =
        crate::app::recover::journalled(&journal, attempt.changes(), ctx.seams.random.as_ref())
            .err()?;
    Some(Outcome::Stopped {
        leaving: format!(
            "what the repair changed stands and could not be recorded, so it cannot be put \
             back: {failure}"
        ),
    })
}

/// Ask again whether the fault is gone.
///
/// Over checks assembled for the purpose rather than the ones that found it: a check holds
/// what it read when it was built, so proving a repair against those very instances would
/// compare its work with the reading it was meant to change — and report every success as
/// a failure.
pub(super) async fn prove(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    again: &[Box<dyn Check>],
    check: &str,
) -> Option<bool> {
    proved(&looked(ctx, services, again).await, check)
}

/// What an attempt and the proof of it amount to together.
///
/// Apart from [`carried`] so that all three answers can be asked of it directly: the one
/// inside a function that assembles nine checks and runs them is an answer no test reaches,
/// and the repo keeps applicable code coverable rather than arguing about it afterwards.
pub(super) fn judged(attempt: Attempt, proof: Option<bool>) -> Outcome {
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
pub(super) fn proved(found: &[Finding], check: &str) -> Option<bool> {
    let Some(finding) = found.iter().find(|finding| finding.check == check) else {
        return Some(true);
    };
    match finding.verdict {
        Verdict::Pass { .. } | Verdict::Skipped { .. } => Some(true),
        Verdict::Warn(_) | Verdict::Fail(_) => Some(false),
        Verdict::Unverified { .. } => None,
    }
}
