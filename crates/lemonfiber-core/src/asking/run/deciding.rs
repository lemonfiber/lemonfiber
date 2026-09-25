//! Ruling on one request that is waiting on somebody.
//!
//! Apart from choosing a policy because they are different errands, and because they
//! refuse for different reasons. Setting a limit fails on a service that will not answer;
//! this one fails on that, on a request nobody is waiting on, on a reason that says
//! nothing, and — for an approval alone — on a disk with no room left.
//!
//! **The disk refuses in the disk's own words.** There is one reading of the volumes in
//! this crate and one refusal built from it, shared by every command that brings content
//! onto the disk, and this is one of them. So a full disk and a spent limit are told
//! apart in the code as well as in the sentence: an operator who read the first as the
//! second would go and raise a quota and watch the same refusal happen again.
//!
//! **A reason is required and the service has nowhere to put it.** Its own endpoint
//! carries the decision in the path and reads no body at all, and its record has no field
//! for one — so what it sends the person who asked is that it was declined, and nothing
//! beside it. Checked against the pinned image rather than assumed; see
//! [`crate::ports::service::Approving::decide`].
//!
//! So the reason is **written down here** rather than said once and dropped. A reason
//! that lived only in the line the operator saw is the silent decline this is here to
//! prevent, arriving one step after the blank field that is refused outright — and the
//! answer to whoever asked is composed from that record, on every reading of the
//! household from then on.
//!
//! **And it is carried to them, where they left an address to carry it to.** What
//! becomes of the words is [`super::passing_on`]'s, kept apart from the decision
//! because the two answer to different things: a decision the service would not take is
//! a refusal to report, and a message that could not go is a line to read.

use crate::error::{Diagnose, Problem};
use crate::household::State;
use crate::model::HouseholdReport;
use crate::ports::service::{Approving as _, HouseholdRequest, Requests as _};

use crate::app::command::{Answer, Decision};
use crate::app::Ctx;

/// Let one waiting request through, or turn it down with the reason it owes.
pub(crate) async fn deciding(
    ctx: &Ctx,
    decision: &Decision,
) -> Result<HouseholdReport, Box<Problem>> {
    let reason = reason_given(&decision.answer)?;
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let access = super::reached(ctx, &manifest.services).await?;
    let asked = access
        .seerr
        .requests()
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_DECIDED)))?;
    let waiting = still_waiting(&asked, decision.request)
        .ok_or_else(|| Box::new(crate::asking::nothing_to_decide(decision.request)))?;

    let approve = matches!(decision.answer, Answer::LetThrough);
    // Only on the way in. A request already approved is content already being fetched,
    // and a disk that filled afterwards is a reason to stop fetching rather than a
    // reason to take back an answer somebody was given.
    if approve {
        crate::space::run::admits(ctx).await?;
    }

    let said = said_of(waiting, reason);
    let mut notes = Vec::new();
    if !ctx.dry_run {
        access
            .seerr
            .decide(decision.request, approve)
            .await
            .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_DECIDED)))?;
        // The operator's own, always: what closes a request nobody ruled on comes through
        // the same door and says whose words it carries there rather than here.
        let words = reason.map(super::passing_on::Said::Operators);
        notes = super::passing_on::carried(ctx, &access, decision.request, words, &asked).await;
    }

    let mut report = crate::household::run::household(ctx, None).await?;
    // The decision first and what became of its words directly under it, ahead of
    // whatever the reading itself could not do: an operator opened this to rule on
    // something, and the answer to that is the line they are looking for.
    let mut leading = vec![if ctx.dry_run {
        format!("{said} — rehearsed, and nothing was sent or decided")
    } else {
        said
    }];
    leading.extend(notes);
    leading.append(&mut report.findings);
    report.findings = leading;
    Ok(report)
}

/// What is said where the service could not be asked or would not rule.
const NOTHING_DECIDED: &str = "nothing was decided";

/// The reason a decline carries, refused where it says nothing.
///
/// An approval carries none and needs none: what an approval owes the person who asked
/// is the thing they asked for.
fn reason_given(answer: &Answer) -> Result<Option<&str>, Box<Problem>> {
    match answer {
        Answer::LetThrough => Ok(None),
        Answer::TurnedDown { reason } if reason.trim().is_empty() => {
            Err(Box::new(crate::asking::no_reason_given()))
        }
        Answer::TurnedDown { reason } => Ok(Some(reason.trim())),
    }
}

/// The request that number names, where it is one nobody has ruled on.
///
/// Nothing for a request already decided, and nothing for one this service does not
/// hold. Both are the same answer to the operator — there is nothing here to rule on —
/// and telling them apart would mean claiming to know which, from a list that is only
/// as complete as the read that built it.
fn still_waiting(asked: &[HouseholdRequest], request: i64) -> Option<&HouseholdRequest> {
    asked.iter().find(|held| {
        held.id == request
            && State::of(held.request_status, held.media_status) == Some(State::WaitingForApproval)
    })
}

/// What the decision comes to, as the line an operator reads it back in.
///
/// The reason is repeated back on a decline, and said to be kept: the service carries
/// none, so a line that dropped it would leave the operator with nothing to pass on, and
/// saying it is kept is what stops them writing it down twice.
///
/// **What it no longer says is who has to carry it.** That is the line underneath, and it
/// is different every time — whether the words reached the person who asked depends on
/// whether they left an address, on whether this machine is allowed to use it, and on
/// whether the service that holds it answered.
fn said_of(waiting: &HouseholdRequest, reason: Option<&str>) -> String {
    let who = &waiting.member;
    match reason {
        None => format!("what {who} asked for was approved and is being fetched"),
        Some(reason) => format!(
            "what {who} asked for was turned down: {reason} — the request service tells \
             them it was declined and carries no reason, so this is kept here"
        ),
    }
}

#[cfg(test)]
mod tests;
