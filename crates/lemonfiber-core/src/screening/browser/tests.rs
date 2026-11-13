use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{play_of, played, reopened};
use crate::app::{Asker, Ctx, PlayFrom};
use crate::pass::{Key, Session, KEY_BYTES, NONCE_BYTES};
use crate::test_support::{media, moved_household};

/// A title on the shelf, by an identifier shaped like the server's.
const FILM: &str = "0123456789abcdef0123456789abcdef";

/// The moment these passes are sealed at.
const NOW: u64 = 1_800_000_000;

/// A key of no randomness, so every pass made with it is the same.
const fn key() -> Key {
    Key::of([9; KEY_BYTES])
}

/// A session on Ada's own account, its token built rather than written.
fn adas() -> Session {
    Session {
        member: "a7f3".to_owned(),
        token: "fe".repeat(16),
    }
}

/// The pass sealing `session` for `item` behind the nonce `nonce`.
fn sealed(item: &str, session: &Session, nonce: u8) -> String {
    key()
        .seal(item, session, [nonce; NONCE_BYTES], NOW)
        .unwrap_or_default()
}

#[test]
fn a_pass_handed_back_opens_only_for_its_own_member_and_title() {
    let ada = media::member("a7f3", "Ada", true);
    let bea = media::member("b8e4", "Bea", true);
    let pass = sealed(FILM, &adas(), 1);
    assert_eq!(reopened(&key(), &ada, FILM, &pass).ok(), Some(adas()));
    for refused in [
        reopened(&key(), &bea, FILM, &pass),
        reopened(&key(), &ada, "fedcba9876543210fedcba9876543210", &pass),
        reopened(&Key::of([8; KEY_BYTES]), &ada, FILM, &pass),
        reopened(&key(), &ada, FILM, &format!("{pass}x")),
    ] {
        assert_eq!(
            refused
                .err()
                .map(|problem| problem.code.to_string())
                .as_deref(),
            Some("PLAY-12")
        );
    }
}

#[test]
fn every_new_pass_names_a_fresh_play_and_a_renewal_names_the_same_one() {
    let first = sealed(FILM, &adas(), 1);
    let second = sealed(FILM, &adas(), 2);
    let play = play_of(&first);
    assert_eq!(play.len(), 32);
    assert!(play
        .chars()
        .all(|digit| matches!(digit, '0'..='9' | 'a'..='f')));
    assert_eq!(play, play_of(&first));
    assert_ne!(play, play_of(&second));
}

fn ctx_over(transport: Arc<Transport>, tag: &str) -> Ctx {
    moved_household(&format!("browser-{tag}"), transport)
}

fn server(routes: Vec<(&'static str, Answer)>) -> Arc<Transport> {
    let mut answered = routes;
    answered.push((
        media::identity("household"),
        media::answered(&[media::member("a7f3", "Ada", true)]),
    ));
    Transport::by_path(answered)
}

#[tokio::test]
async fn a_browser_named_by_something_else_is_refused_before_anything_is_asked() {
    let transport = server(Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "not-a-device");
    let answered = played(
        &ctx,
        "Ada",
        FILM,
        PlayFrom::Device("a\"b".to_owned()),
        Asker::Member,
    )
    .await;
    assert_eq!(
        answered
            .err()
            .map(|problem| problem.code.to_string())
            .as_deref(),
        Some("PLAY-3")
    );
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn with_nowhere_to_play_it_no_session_is_opened_and_a_member_hears_why_in_household_words() {
    let transport = server(Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "unlocated");
    let answered = played(
        &ctx,
        "Ada",
        FILM,
        PlayFrom::Device("a-browser-0123".to_owned()),
        Asker::Member,
    )
    .await;
    let Ok(report) = answered else {
        unreachable!("answered as unlocated");
    };
    assert_eq!(
        report.unlocated.as_deref(),
        Some(crate::screening::door::told(Asker::Member, String::new()).as_str())
    );
    assert!(report.stream_from.is_none());
    assert!(!transport
        .requests()
        .iter()
        .any(|request| request.url.ends_with("/signed_in")));
}
