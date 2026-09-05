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

use std::collections::BTreeSet;

use crate::asking::Reasons;
use crate::config::REACH_HOUSEHOLD_KEY;
use crate::ports::service::{HouseholdRequest, Telling as _};
use crate::telling::{tell, Told};

use crate::app::targets::HouseholdAccess;
use crate::app::Ctx;

/// What is said where the words are the operator's to carry.
const YOURS: &str = "so the reason is yours to pass on";

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
pub(super) async fn carried(
    ctx: &Ctx,
    access: &HouseholdAccess,
    request: i64,
    reason: Option<&str>,
    asked: &[HouseholdRequest],
) -> Vec<String> {
    let Some(reason) = reason else {
        return Vec::new();
    };
    let mut reasons = held(ctx, request, reason, asked);
    let mut said: Vec<String> = passed_on(ctx, access, request, &mut reasons)
        .await
        .into_iter()
        .collect();
    said.extend(written(ctx, &reasons));
    said
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
fn held(ctx: &Ctx, request: i64, reason: &str, asked: &[HouseholdRequest]) -> Reasons {
    let still_held: BTreeSet<i64> = asked.iter().map(|filed| filed.id).collect();
    let mut reasons = crate::app::refusals::load(ctx);
    reasons.keep(request, reason, crate::instant::written(ctx.clock.now()));
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
mod tests {
    use super::{became_of, held, still_owed, written};
    use crate::asking::Reasons;
    use crate::ports::service::HouseholdRequest;
    use crate::telling::Told;
    use crate::test_support::a_context;

    /// A moment the calendar holds, for these records to be stamped with.
    const AT: &str = "2026-08-17T21:04:09";

    /// One request as the service records it, at the two statuses that decide its state.
    fn asked(id: i64) -> HouseholdRequest {
        HouseholdRequest {
            id,
            made: Some(AT.to_owned()),
            member: "Ana".to_owned(),
            kind: Some(crate::recyclarr::Kind::Radarr),
            item: None,
            request_status: 1,
            media_status: 2,
        }
    }

    /// What one delivery came to, built from the two lists it is.
    fn told(reached: &[&str], refused: &[&str]) -> Told {
        Told {
            reached: reached.iter().map(|at| (*at).to_owned()).collect(),
            refused: refused.iter().map(|at| (*at).to_owned()).collect(),
        }
    }

    /// Reaching them says so and stops claiming the words are still owed.
    #[test]
    fn reaching_them_says_so_and_leaves_nothing_owed() {
        let both = became_of(&told(&["Pushover", "Pushbullet"], &[]));

        assert!(both.contains("Pushover and Pushbullet"), "{both}");
        assert!(!both.contains("yours to pass on"), "{both}");
    }

    /// Nowhere to send is said as the ordinary thing it is, not as a fault.
    #[test]
    fn nowhere_to_send_is_said_as_an_absence_rather_than_a_failure() {
        let nowhere = became_of(&told(&[], &[]));

        assert!(nowhere.contains("no address"), "{nowhere}");
        assert!(nowhere.contains("yours to pass on"), "{nowhere}");
    }

    /// Everything refusing leaves the words with the operator, and names what refused.
    #[test]
    fn everything_refusing_leaves_the_words_with_the_operator() {
        let refused = became_of(&told(&[], &["Pushover"]));

        assert!(
            refused.starts_with("Pushover would not take it"),
            "{refused}"
        );
        assert!(refused.contains("yours to pass on"), "{refused}");
    }

    /// One taking it and one refusing is told once, and said as told once.
    #[test]
    fn one_taking_it_and_one_refusing_is_told_once() {
        let mixed = became_of(&told(&["Pushbullet"], &["Pushover"]));

        assert!(mixed.contains("on Pushbullet"), "{mixed}");
        assert!(mixed.contains("Pushover would not take it"), "{mixed}");
        assert!(mixed.contains("once rather than twice"), "{mixed}");
        assert!(!mixed.contains("yours to pass on"), "{mixed}");
    }

    /// Words already carried are owed to nobody, and a request nobody refused here
    /// owes nothing at all.
    #[test]
    fn words_carried_once_are_owed_to_nobody() {
        let mut reasons = Reasons::default();
        assert_eq!(still_owed(&reasons, 7), None, "a refusal nobody made");

        reasons.keep(7, "no room this month", None);
        assert_eq!(
            still_owed(&reasons, 7),
            Some("no room this month".to_owned())
        );

        reasons.passed_on(7, vec!["Pushover".to_owned()], None);
        assert_eq!(still_owed(&reasons, 7), None, "the same words twice");
    }

    /// The record keeps this refusal, trimmed, and only what the service still holds.
    #[test]
    fn the_record_keeps_this_refusal_and_only_what_the_service_still_holds() {
        let nowhere = a_context().build();

        let kept = held(&nowhere, 7, "  no room this month  ", &[asked(7)]);
        assert_eq!(
            kept.of(7).map(|refusal| refusal.reason.as_str()),
            Some("no room this month")
        );

        let gone = held(&nowhere, 7, "no room this month", &[asked(9)]);
        assert!(
            gone.is_empty(),
            "a reason whose request the service does not hold was kept"
        );
    }

    /// A reason nowhere could hold is said to be in this answer alone.
    ///
    /// The decision itself went through, so this is not a failure — but an operator who
    /// closed the window believing the words were kept would find the next reading of
    /// the household bare, and the person who asked would never hear why.
    #[test]
    fn a_reason_that_could_not_be_kept_says_where_it_now_lives() {
        let nowhere = a_context().build();

        let said = written(&nowhere, &Reasons::default());

        assert!(
            said.is_some_and(|said| said.contains("nowhere else")),
            "a reason nothing kept was reported as kept"
        );
    }
}
