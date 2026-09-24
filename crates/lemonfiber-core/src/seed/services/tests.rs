use super::{said, tell_the_household, wanted_telling, TELLING};
use crate::baseline::Baseline;
use crate::seed::drift::{intent, Intent, Observed};
use crate::seed::State;
use crate::seerr::Seerr;
use lemonfiber_fixtures::http::{Answer, Fake};
use std::sync::Arc;

/// A telling that is on, but not for the occasions lemonfiber would choose.
const SOME: &str = r#"{"enabled":true,"types":8}"#;

/// The real client over a scripted service, so these read the request this
/// product actually sends rather than a second description of it.
fn service(answers: Vec<Answer>) -> (Seerr, Arc<Fake>) {
    let http = Fake::by_path_in_turn(vec![("/settings/notifications/webpush", answers)]);
    (Seerr::new(http.clone(), "http://seerr:5055", "seerr"), http)
}

/// A baseline holding one value for the telling.
fn recorded(value: &str, adopted: bool) -> Baseline {
    let mut baseline = Baseline::new();
    if adopted {
        baseline.adopt("seerr", TELLING, value, "2026-08-28T00:00:00Z");
    } else {
        baseline.record("seerr", TELLING, value, "2026-08-28T00:00:00Z");
    }
    baseline
}

async fn against(seerr: &Seerr, baseline: &Baseline) -> State {
    tell_the_household(seerr, baseline.entry("seerr", TELLING), false)
        .await
        .0
}

/// The same comparison over the same service, asked what the pass would do rather
/// than asked to do it.
async fn would(seerr: &Seerr, baseline: &Baseline) -> State {
    tell_the_household(seerr, baseline.entry("seerr", TELLING), true)
        .await
        .0
}

/// Whether the service was asked to change anything.
fn written_to(http: &Fake) -> bool {
    http.requests()
        .iter()
        .any(|asked| asked.method == crate::ports::http::Method::Post)
}

#[tokio::test]
async fn a_service_holding_what_was_wanted_is_left_exactly_as_it_is() {
    let held = format!(
        r#"{{"enabled":true,"types":{}}}"#,
        wanted_telling().occasions
    );
    let (seerr, http) = service(vec![Answer::reply(200, held)]);

    let state = against(&seerr, &recorded(&said(wanted_telling()), false)).await;

    assert_eq!(state, State::AlreadyWired);
    assert!(!written_to(&http), "a correct value was written again");
}

#[tokio::test]
async fn lemonfibers_own_value_behind_its_intent_is_reported_rather_than_rewritten() {
    // The baseline and the service agree; it is lemonfiber that has moved on.
    let (seerr, http) = service(vec![Answer::reply(200, SOME)]);

    let state = against(&seerr, &recorded("on:8", false)).await;

    assert_eq!(state, State::Stale);
    assert!(!written_to(&http), "a value nobody edited was overwritten");
}

#[tokio::test]
async fn both_sides_moved_is_put_to_the_operator_rather_than_settled() {
    // The baseline matches neither what the service holds nor what is wanted.
    let (seerr, http) = service(vec![Answer::reply(200, SOME)]);

    let state = against(&seerr, &recorded("on:2", false)).await;

    assert!(
        matches!(&state, State::Conflicted { yours, ours }
            if yours.as_deref() == Some("on:8") && ours == &said(wanted_telling())),
        "{state:?}"
    );
    assert!(!written_to(&http), "a conflict was resolved by writing");
}

#[tokio::test]
async fn a_value_the_operator_had_adopted_stays_theirs() {
    let (seerr, http) = service(vec![Answer::reply(200, SOME)]);

    let state = against(&seerr, &recorded("on:8", true)).await;

    assert_eq!(state, State::Adopted);
    assert!(!written_to(&http));
}

#[tokio::test]
async fn a_value_set_before_lemonfiber_ever_ran_is_taken_on_rather_than_flagged() {
    // Something is set, and lemonfiber never wrote it: theirs, pre-existing.
    let (seerr, http) = service(vec![Answer::reply(200, SOME)]);

    let state = against(&seerr, &Baseline::new()).await;

    assert_eq!(state, State::Unmanaged);
    assert!(!written_to(&http), "a pre-existing value was overwritten");
}

#[tokio::test]
async fn a_service_that_will_not_answer_is_reported_rather_than_guessed_at() {
    let (seerr, http) = service(vec![Answer::Silent]);

    let state = against(&seerr, &Baseline::new()).await;

    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
    assert!(
        !written_to(&http),
        "a service that would not answer was written to anyway"
    );
}

/// A write that does not land is reported, not assumed.
///
/// The household hears nothing either way; the difference is whether the operator
/// is told. A pass that reported `Wired` on a refused write would leave them
/// believing the loop closes.
#[tokio::test]
async fn a_write_the_service_refuses_is_reported_in_its_own_words() {
    let (seerr, _) = service(vec![
        Answer::reply(200, r#"{"enabled":false,"types":0}"#),
        Answer::reply(500, "no"),
    ]);

    let state = against(&seerr, &Baseline::new()).await;

    assert!(
        matches!(state, State::Failed { .. } | State::Skipped { .. }),
        "a refused write was not reported: {state:?}"
    );
}

/// A rehearsal says what the household hears now and what it would be signed up
/// for, and signs them up for nothing.
///
/// Both halves are the answer to the question that was asked. A report naming only
/// the connection would leave the operator deciding about a phrase; what they are
/// actually deciding about is that nobody in the house hears anything today and
/// that a real run would have the service write to them on every occasion
/// lemonfiber sends on. And the write is the one thing a question must not make: a
/// household that started being told because somebody asked what would happen was
/// told by the asking, and there is no undoing a notification that has gone out.
#[tokio::test]
async fn a_rehearsed_telling_says_what_is_set_now_and_sets_nothing() {
    // A service nobody has configured, with nothing recorded against it — the one
    // case that is a connection to make rather than somebody's own choice to leave
    // alone, and so the only one with anything to report in the other tense.
    let (seerr, http) = service(vec![Answer::reply(200, r#"{"enabled":false,"types":0}"#)]);

    let state = would(&seerr, &Baseline::new()).await;

    assert_eq!(
        state,
        State::WouldWire {
            yours: Some("off:0".to_owned()),
            ours: Some(said(wanted_telling())),
        }
    );
    assert!(
        !written_to(&http),
        "a question about the household told the household"
    );
}

/// The states above are the shared policy, not a second opinion about it.
///
/// This maps the observation straight to a state rather than going through
/// `intent`, because one of `intent`'s outcomes cannot arise here and an arm
/// nothing reaches is an arm nothing checks. The correspondence is asserted
/// instead, so the two cannot drift apart in silence.
#[test]
fn every_state_here_is_the_one_the_shared_policy_asks_for() {
    let paired = [
        (Observed::Absent, Intent::Wire),
        (Observed::Present, Intent::Leave),
        (Observed::Drifted, Intent::Preserve),
        (Observed::Stale, Intent::Update),
        (Observed::Conflicted, Intent::Ask),
        (Observed::Adopted, Intent::Keep),
        (Observed::Unmanaged, Intent::Adopt),
    ];
    for (observed, expected) in paired {
        assert_eq!(
            intent(observed),
            expected,
            "the telling treats {observed:?} as {expected:?}, and the shared policy \
             no longer agrees"
        );
    }
}
