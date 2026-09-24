//! What becomes of a refusal's reason: written down, carried to whoever asked, once.
//!
//! Apart from the decision it follows because they answer to different things. The
//! decision is the request service's and either goes through or is reported as not
//! having; this is a message to a person, and every way it can come to nothing is an
//! ordinary outcome to be said out loud rather than a failure to undo an answer over.
//!
//! **The words go once and only once.** The record beside the reason says whether the
//! attempt has been made, and it is written whether anything was reached or not — so a
//! household is never told the same thing twice, and a member with nowhere to send to is
//! asked about once and then left alone. That is the rule the operator's own alerts are
//! held to; this is the household's half of it.
//!
//! **Every outcome is a line the operator reads.** Whether the reason reached them
//! decides whether passing it on is still theirs to do, and that is not something to
//! leave them guessing at: a refusal answered with silence about its own delivery reads
//! as delivered.
//!
//! **Two things now come through here, and they carry the same message and a different
//! record.** An operator turning something down, and a period the household agreed to
//! closing something nobody ruled on. What travels to the person who asked is why, in both
//! cases, because that is the one thing the request service has nowhere to put — but the
//! record keeps which of the two it was, because having been refused and having run out
//! are different things to have happened to somebody.

use std::collections::BTreeSet;

use crate::asking::Reasons;
use crate::config::REACH_HOUSEHOLD_KEY;
use crate::ports::service::{Addressing as _, HouseholdRequest};
use crate::telling::{tell, Told};

use crate::app::targets::HouseholdAccess;
use crate::app::Ctx;

/// What is said where the words are the operator's to carry.
const YOURS: &str = "so the reason is yours to pass on";

/// What was said when one request was turned down, and whose words they are.
///
/// **Two shapes rather than a string beside a flag**, because the two combinations a flag
/// would also allow are not things: an approval carries no reason at all, and a request
/// that ran out cannot have carried somebody's own. Having none is the absence of one of
/// these rather than a third kind of one, which is why an approval is `None` at the call
/// below and not a variant here — a variant would be a state every reader had to rule out
/// and no caller could ever construct past the check that returns early on it.
///
/// What travels to the person who asked is the same either way — the request service tells
/// them it was declined and this carries only why — but what the record keeps is not, and
/// neither is what the member is told afterwards.
#[derive(Clone, Copy)]
pub(crate) enum Said<'a> {
    /// The operator's own words, on a refusal they made.
    Operators(&'a str),
    /// This program's own, on a request nobody ruled on inside the period the household
    /// agreed to let one wait.
    RanOut(&'a str),
}

/// What is said where the words could not be written down.
///
/// The decision itself went through, so this is not a failure to report as one — but the
/// reason is now in this answer and nowhere else, and an operator who closed the window
/// believing it was kept would find the next reading of the household bare.
const NOT_KEPT: &str = "the reason could not be written down here, so it is in this answer \
                        and nowhere else — copy it before you close this, because the \
                        request service keeps none either";

/// Write the reason down, carry it to whoever asked, and say what became of both.
///
/// Only after the service has taken the decision: a reason recorded for a refusal that
/// never happened would be shown to somebody beside a request that is still waiting.
///
/// Nothing on an approval — there is no reason to keep, nothing to tell anybody, and
/// clearing the record on one would lose the words for every *other* refusal in the same
/// breath.
pub(crate) async fn carried(
    ctx: &Ctx,
    access: &HouseholdAccess,
    request: i64,
    said: Option<Said<'_>>,
    asked: &[HouseholdRequest],
) -> Vec<String> {
    let Some(said) = said else {
        return Vec::new();
    };
    let mut reasons = held(ctx, request, said, asked);
    let mut notes: Vec<String> = passed_on(ctx, access, request, &mut reasons)
        .await
        .into_iter()
        .collect();
    notes.extend(written(ctx, &reasons));
    notes
}

/// Put the record where the next run will find it, and say so where it would not go.
fn written(ctx: &Ctx, reasons: &Reasons) -> Option<String> {
    crate::app::refusals::keep(ctx, reasons)
        .is_err()
        .then(|| NOT_KEPT.to_owned())
}

/// Every reason this machine holds, with this one added and the gone ones dropped.
///
/// The pruning rides along because this is the one path that holds both halves at once:
/// the record, and the service's own list of what still exists. A note beside a line that
/// has gone is only a way to grow a file forever.
fn held(ctx: &Ctx, request: i64, said: Said<'_>, asked: &[HouseholdRequest]) -> Reasons {
    let still_held: BTreeSet<i64> = asked.iter().map(|filed| filed.id).collect();
    let mut reasons = crate::app::refusals::load(ctx);
    let at = crate::instant::written(ctx.clock.now());
    match said {
        Said::Operators(reason) => reasons.keep(request, reason, at),
        Said::RanOut(reason) => reasons.closed(request, reason, at),
    }
    reasons.only(&still_held);
    reasons
}

/// Carry the reason for one refusal to whoever asked, and say what became of it.
///
/// Nothing where the words have already been carried: a second telling is not a thing
/// this can do, and a second line about one would report something that did not happen.
async fn passed_on(
    ctx: &Ctx,
    access: &HouseholdAccess,
    request: i64,
    reasons: &mut Reasons,
) -> Option<String> {
    let reason = still_owed(reasons, request)?;
    if !ctx.settings.reaching.allows(REACH_HOUSEHOLD_KEY) {
        return Some(format!(
            "nothing was sent to them — {REACH_HOUSEHOLD_KEY} is off, {YOURS}"
        ));
    }
    let Ok(addresses) = access.seerr.reachable(request).await else {
        return Some(format!(
            "where they are reached could not be read from the request service, {YOURS}"
        ));
    };
    let told = tell(&ctx.http, &addresses, &reason).await;
    reasons.passed_on(
        request,
        told.reached.clone(),
        crate::instant::written(ctx.clock.now()),
    );
    Some(became_of(&told))
}

/// The words this refusal still owes whoever asked, or nothing where it owes none.
fn still_owed(reasons: &Reasons, request: i64) -> Option<String> {
    let kept = reasons.of(request)?;
    kept.told.is_none().then(|| kept.reason.clone())
}

/// What became of the sending, as the operator reads it.
fn became_of(told: &Told) -> String {
    if told.nowhere() {
        return format!(
            "the request service holds no address for them this could send to, {YOURS}"
        );
    }
    let refused = told.refused.join(" and ");
    if told.reached.is_empty() {
        return format!("{refused} would not take it, {YOURS}");
    }
    let reached = told.reached.join(" and ");
    if told.refused.is_empty() {
        return format!("they have been told why, on {reached}");
    }
    format!(
        "they have been told why, on {reached} — {refused} would not take it, so they have \
         it once rather than twice"
    )
}

#[cfg(test)]
mod tests;
