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
//! **A reason is required and does not reach the requester.** The request service's own
//! endpoint carries the decision in the path and reads no body at all, and its record has
//! no field for one — so what it sends the person who asked is that it was declined, and
//! the reason stays here for the operator to pass on. Checked against the pinned image
//! rather than assumed; see [`crate::ports::service::Approving::decide`].

use crate::error::{Diagnose, Problem};
use crate::household::State;
use crate::model::HouseholdReport;
use crate::ports::service::{Approving as _, HouseholdRequest, Requests as _};

use crate::app::command::{Answer, Decision};
use crate::app::Ctx;

/// Let one waiting request through, or turn it down with the reason it owes.
pub(in crate::app) async fn deciding(
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
        crate::app::space::admits(ctx).await?;
    }

    let said = said_of(waiting, reason);
    if !ctx.dry_run {
        access
            .seerr
            .decide(decision.request, approve)
            .await
            .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_DECIDED)))?;
    }

    let mut report = super::super::household::household(ctx, None).await?;
    report.findings.insert(
        0,
        if ctx.dry_run {
            format!("{said} — rehearsed, and nothing was decided")
        } else {
            said
        },
    );
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
/// The reason is repeated back on a decline. It reaches nobody else — the service
/// carries none — so the operator is the one who has to pass it on, and a line that
/// dropped it would leave them with nothing to pass on.
fn said_of(waiting: &HouseholdRequest, reason: Option<&str>) -> String {
    let who = &waiting.member;
    match reason {
        None => format!("what {who} asked for was approved and is being fetched"),
        Some(reason) => format!(
            "what {who} asked for was turned down: {reason} — the request service tells \
             them it was declined and carries no reason, so this is yours to pass on"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{reason_given, said_of, still_waiting};
    use crate::app::command::Answer;
    use crate::ports::service::HouseholdRequest;

    /// One request as the service records it, at the two statuses that decide its state.
    fn asked(id: i64, request_status: u8, media_status: u8) -> HouseholdRequest {
        HouseholdRequest {
            id,
            made: Some("2026-08-17T21:04:09".to_owned()),
            member: "Ana".to_owned(),
            kind: Some(crate::recyclarr::Kind::Radarr),
            item: None,
            request_status,
            media_status,
        }
    }

    /// A request nobody has ruled on is the one that can be ruled on.
    #[test]
    fn a_request_nobody_has_ruled_on_is_the_one_that_can_be_decided() {
        let held = [asked(7, 1, 1)];

        assert!(still_waiting(&held, 7).is_some());
    }

    /// One already decided, and one this service does not hold, are both nothing.
    #[test]
    fn one_already_decided_and_one_it_does_not_hold_are_both_nothing() {
        let held = [asked(7, 2, 5), asked(8, 3, 1)];

        assert!(
            still_waiting(&held, 7).is_none(),
            "approved, and decided again"
        );
        assert!(
            still_waiting(&held, 8).is_none(),
            "declined, and decided again"
        );
        assert!(still_waiting(&held, 99).is_none());
        assert!(still_waiting(&[], 7).is_none());
    }

    /// An approval carries no reason and needs none.
    #[test]
    fn an_approval_carries_no_reason_and_needs_none() {
        assert_eq!(reason_given(&Answer::LetThrough).unwrap_or(Some("x")), None);
    }

    /// A decline's reason is trimmed, and one that says nothing is refused.
    #[test]
    fn a_reason_that_says_nothing_is_refused() {
        // Bound rather than passed inline: the answer owns the reason, and a temporary
        // built in the call would be gone before what it lent out was read.
        let padded = Answer::TurnedDown {
            reason: "  the disk is nearly full  ".to_owned(),
        };
        let given = reason_given(&padded);
        assert_eq!(given.unwrap_or(None), Some("the disk is nearly full"));

        for blank in ["", "   ", "\t\n"] {
            let empty = Answer::TurnedDown {
                reason: blank.to_owned(),
            };
            let refused = reason_given(&empty);
            assert_eq!(
                refused.err().map(|problem| problem.code),
                Some(crate::asking::NO_REASON),
                "{blank:?} was accepted"
            );
        }
    }

    /// A decline says the reason back, and says who has to carry it.
    ///
    /// It reaches nobody else — the request service carries none — so a line that
    /// dropped it would leave the operator with nothing to pass on.
    #[test]
    fn a_decline_says_the_reason_back_and_who_has_to_carry_it() {
        let said = said_of(&asked(7, 1, 1), Some("we are out of room"));

        assert!(said.contains("Ana"), "{said}");
        assert!(said.contains("we are out of room"), "{said}");
        assert!(said.contains("yours to pass on"), "{said}");
    }

    /// An approval says it is being fetched, and says nothing about a reason.
    #[test]
    fn an_approval_says_it_is_being_fetched() {
        let said = said_of(&asked(7, 1, 1), None);

        assert!(said.contains("Ana"), "{said}");
        assert!(said.contains("approved"), "{said}");
        assert!(!said.contains("pass on"), "{said}");
    }
}
