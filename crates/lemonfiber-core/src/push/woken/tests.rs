//! A wake carries an id, and only the phone it went to confirms it.

use std::path::{Path, PathBuf};

use super::{confirmed, sent};
use crate::alert::Notification;
use crate::app::Ctx;
use crate::config::Settings;
use crate::error::codes::notify::NO_SUCH_WAKE;
use crate::model::DeliveryOutcome;
use crate::push::{Kept, Record};

fn record(name: &str, owner: Option<&str>) -> Record {
    Record {
        name: name.to_owned(),
        endpoint: "https://push.example/a".to_owned(),
        p256dh: String::new(),
        auth: String::new(),
        registered: String::new(),
        owed: 0,
        owner: owner.map(str::to_owned),
        wakes: Vec::new(),
        last: None,
        failures: Vec::new(),
        proven: None,
    }
}

fn machine(name: &str) -> (Ctx, PathBuf) {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    let ctx = crate::test_support::a_context()
        .settings(Settings {
            env_file: Some(dir.join("config").join(".env")),
            stack_dir: Some(dir.join("data").join("stack")),
            ..Settings::default()
        })
        .build();
    let path = crate::push::run::kept_at(&ctx).unwrap_or_default();
    let _ = crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
        kept.endpoints = vec![record("operator", None), record("ada-phone", Some("ada"))];
        Ok(())
    });
    (ctx, path)
}

fn endpoint(path: &Path, name: &str) -> Option<Record> {
    crate::push::run::read_kept(path)
        .ok()?
        .endpoints
        .into_iter()
        .find(|one| one.name == name)
}

#[test]
fn a_wake_taken_waits_for_its_phone_and_one_refused_is_a_failure_kept() {
    let (_, path) = machine("push-wakes-sent");

    sent(&path, "ada-phone", "w1", &DeliveryOutcome::Sent, 10);
    sent(
        &path,
        "operator",
        "w2",
        &DeliveryOutcome::Failed("410".to_owned()),
        11,
    );

    let ada = endpoint(&path, "ada-phone");
    assert_eq!(
        ada.as_ref().map(|one| one.wakes.clone()),
        Some(vec!["w1".to_owned()])
    );
    assert_eq!(
        ada.and_then(|one| one.last).map(|last| last.kind),
        Some(Notification::Ready)
    );
    let operator = endpoint(&path, "operator");
    assert_eq!(operator.as_ref().map(|one| one.failures.len()), Some(1));
    assert!(operator.is_some_and(|one| one.wakes.is_empty()));
}

#[test]
fn only_whoever_registered_the_phone_confirms_its_wake_and_only_once() {
    let (ctx, path) = machine("push-wakes-confirmed");
    sent(&path, "ada-phone", "w1", &DeliveryOutcome::Sent, 10);

    let stranger = confirmed(&ctx, "ada-phone", "w1", Some("bo"));
    let operator = confirmed(&ctx, "ada-phone", "w1", None);
    let unknown = confirmed(&ctx, "ada-phone", "w9", Some("ada"));
    let own = confirmed(&ctx, "ada-phone", "w1", Some("ada"));
    let again = confirmed(&ctx, "ada-phone", "w1", Some("ada"));

    for refused in [stranger, operator, unknown, again] {
        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(NO_SUCH_WAKE)
        );
    }
    assert!(own.is_ok());
    let ada = endpoint(&path, "ada-phone");
    assert!(ada.as_ref().is_some_and(|one| one.proven.is_some()));
    assert_eq!(
        ada.and_then(|one| one.last).map(|last| last.outcome),
        Some(DeliveryOutcome::Confirmed)
    );
}
