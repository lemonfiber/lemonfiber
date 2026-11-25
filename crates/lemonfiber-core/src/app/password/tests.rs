use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::support::a_password;

use crate::admission as credential;
use crate::app::{dispatch, Command, Ctx, Outcome, Rehearsal};
use crate::error::codes::admit::MISTYPED;
use crate::test_support::{a_context, Typing};

use super::{PasswordReport, AGAIN, ASKS};

fn keeping(at: Option<PathBuf>, typing: &Arc<Typing>) -> Ctx {
    a_context()
        .settings(crate::config::Settings {
            admission: at,
            ..crate::config::Settings::default()
        })
        .build()
        .with_asking(Arc::clone(typing) as Arc<dyn crate::ports::asking::Asking>)
}

fn kept_at(name: &str) -> (PathBuf, lemonfiber_fixtures::scratch::Scratch) {
    let root = lemonfiber_fixtures::scratch::Scratch::new(name);
    (root.join("admission.json"), root)
}

#[tokio::test]
async fn a_password_typed_twice_is_kept_and_the_device_keys_under_the_last_one_revoked() {
    let (at, _root) = kept_at("password-kept");
    let chosen = a_password();
    let typing = Typing::answering_each(vec![chosen.clone(), chosen.clone()]);

    let outcome = dispatch(Command::Password, &keeping(Some(at.clone()), &typing)).await;

    assert_eq!(
        outcome,
        Ok(Outcome::Password(PasswordReport {
            device_keys_revoked: true
        }))
    );
    assert!(credential::at(&at).is_some_and(|held| held.verifies(&chosen)));
    assert_eq!(
        typing.asked(),
        vec![(ASKS.to_owned(), true), (AGAIN.to_owned(), true)]
    );
}

#[tokio::test]
async fn two_answers_that_differ_are_refused_and_nothing_is_kept() {
    let (at, _root) = kept_at("password-mistyped");
    let chosen = a_password();
    let typing = Typing::answering_each(vec![chosen.clone(), chosen.to_uppercase()]);

    let refused = dispatch(Command::Password, &keeping(Some(at.clone()), &typing)).await;

    assert_eq!(refused.err().map(|problem| problem.code), Some(MISTYPED));
    assert_eq!(credential::at(&at), None);
}

#[tokio::test]
async fn a_password_with_nowhere_to_be_kept_or_too_short_is_refused() {
    let short: String = a_password().chars().take(3).collect();
    let (at, _root) = kept_at("password-short");

    for (kept, typed) in [(None, a_password()), (Some(at.clone()), short)] {
        let typing = Typing::answering_each(vec![typed]);
        let refused = dispatch(Command::Password, &keeping(kept, &typing)).await;
        assert!(refused.is_err(), "{refused:?}");
    }
    assert_eq!(credential::at(&at), None);
}

#[tokio::test]
async fn nowhere_to_keep_a_password_asks_for_none() {
    let typing = Typing::answering_each(vec![a_password()]);

    let refused = dispatch(Command::Password, &keeping(None, &typing)).await;

    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(typing.asked(), Vec::new());
}

#[test]
fn setting_a_password_cannot_be_rehearsed() {
    let asked = Command::Password.asked();
    assert!(matches!(asked.rehearsal, Rehearsal::Cannot(_)));
}
