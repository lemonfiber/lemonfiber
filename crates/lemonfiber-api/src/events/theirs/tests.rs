use std::sync::Arc;
use std::time::Duration;

use lemonfiber_core::app::{Command, Ctx};
use lemonfiber_core::keys::Scope;
use lemonfiber_core::model::kind;
use tokio::time::Instant;

use super::{due, unread, Theirs, PLAYING_EVERY, ROWS_EVERY, UNREAD};
use crate::admission::{Caller, Keyed};
use crate::events::live::Gathers;

/// The member asking, by the id the media server files them under.
const ASKING: &str = "a7f3";

/// A world whose media server nobody can sign in to, so every reading is unread.
fn a_world() -> Arc<Ctx> {
    Arc::new(lemonfiber_testing::a_context().build())
}

fn a_member() -> Caller {
    Caller::Member(ASKING.to_owned())
}

/// The kinds a gather said, in the order it said them.
fn kinds(said: &[crate::events::wire::Rendered]) -> Vec<&'static str> {
    said.iter().map(|one| one.kind().as_str()).collect()
}

/// Joining says all three of the member's own, and nothing of anybody else's: no
/// dashboard, no news, nothing the operator's stream carries.
#[tokio::test(start_paused = true)]
async fn joining_says_the_members_row_their_shelf_and_what_they_are_playing() {
    let theirs = Theirs::for_member(a_world(), a_member());
    let said = theirs.gather(true).await;
    assert_eq!(
        kinds(&said),
        vec![
            kind::HOUSEHOLD.as_str(),
            kind::HELD.as_str(),
            kind::PLAYING.as_str()
        ]
    );
}

/// **Each is the member's own.** What is said names the member asking, because the
/// commands were narrowed by the decision every read takes.
#[tokio::test(start_paused = true)]
async fn what_is_said_is_narrowed_to_the_member() {
    let theirs = Theirs::for_member(a_world(), a_member());
    for one in theirs.gather(true).await {
        let named = one.said().contains(ASKING);
        let whole_house = one.kind() == kind::PLAYING && one.said().contains(r#""member":"""#);
        assert!(
            named || one.kind() == kind::HOUSEHOLD,
            "{} was not narrowed to the member: {}",
            one.kind(),
            one.said()
        );
        assert!(!whole_house, "what is playing was read for the whole house");
    }
}

/// A key scoped to a member hears what that member hears.
#[tokio::test(start_paused = true)]
async fn a_member_key_hears_what_the_member_hears() {
    let keyed = Caller::Key(Keyed {
        name: "home-assistant".to_owned(),
        scope: Scope::Member {
            id: ASKING.to_owned(),
            name: "ana".to_owned(),
        },
    });
    let by_key = Theirs::for_member(a_world(), keyed).gather(true).await;
    let by_session = Theirs::for_member(a_world(), a_member()).gather(true).await;
    assert_eq!(kinds(&by_key), kinds(&by_session));
}

/// A caller who is not a member hears nothing on a member's stream, rather than the
/// household's whole view the commands would answer as asked.
#[tokio::test(start_paused = true)]
async fn a_caller_who_is_not_a_member_hears_nothing() {
    for caller in [Caller::Operator, Caller::Machine] {
        assert!(Theirs::for_member(a_world(), caller)
            .gather(true)
            .await
            .is_empty());
    }
}

/// Between paces nothing is read again, and an answer that has not changed is not said
/// again when its pace comes round.
#[tokio::test(start_paused = true)]
async fn nothing_is_said_again_until_it_is_due_and_changed() {
    let theirs = Theirs::for_member(a_world(), a_member());
    let _joined = theirs.gather(true).await;
    assert!(
        theirs.gather(false).await.is_empty(),
        "read again before it was due"
    );

    tokio::time::advance(PLAYING_EVERY).await;
    assert!(
        theirs.gather(false).await.is_empty(),
        "an unchanged reading was said again"
    );

    tokio::time::advance(ROWS_EVERY).await;
    assert!(theirs.gather(false).await.is_empty());
}

/// A pace comes round once its interval has passed, and at once when nothing was read.
#[test]
fn a_pace_comes_round_once_its_interval_has_passed() {
    let now = Instant::now();
    assert!(due(None, ROWS_EVERY, now));
    assert!(!due(Some(now), ROWS_EVERY, now));
    assert!(due(Some(now), ROWS_EVERY, now + ROWS_EVERY));
    assert!(!due(
        Some(now),
        ROWS_EVERY,
        now + ROWS_EVERY - Duration::from_millis(1)
    ));
}

/// A reading the stack could not make is said as unread with a sentence of its own,
/// never as an empty household, an empty shelf or a quiet house — and never with the
/// operator's problem, which names what only the operator is shown.
#[test]
fn an_unmade_reading_is_unread_in_a_sentence_of_its_own() {
    use lemonfiber_core::app::{Outcome, Whom};
    let household = unread(&Command::Household { member: None });
    assert!(matches!(&household, Some(Outcome::Household(report))
        if !report.available && report.findings == vec![UNREAD.to_owned()]));
    let held = unread(&Command::Held {
        member: Whom::Defaults,
        most: 1,
    });
    assert!(matches!(&held, Some(Outcome::Held(report))
        if !report.available && report.findings == vec![UNREAD.to_owned()]));
    let playing = unread(&Command::Playing { member: None });
    assert!(matches!(&playing, Some(Outcome::Playing(report))
        if !report.available && report.findings == vec![UNREAD.to_owned()]));
    assert!(unread(&Command::Version).is_none());
}
