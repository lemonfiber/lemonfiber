use super::{load, save};
use crate::alert::{Alert, Moment, Outbox};
use crate::test_support::a_context;

/// A context whose environment file is in an emptied scratch directory.
fn ctx_at(name: &str) -> crate::app::Ctx {
    let dir = std::env::temp_dir().join(format!("lemonfiber-outbox-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .settings(crate::config::Settings {
            env_file: Some(dir.join(".env")),
            ..crate::config::Settings::default()
        })
        .build()
}

/// One alert about the given check.
fn alert(check: &str) -> Alert {
    Alert {
        check: check.to_owned(),
        kind: "service.down".to_owned(),
        moment: Moment::Onset,
        severity: crate::error::Severity::Warning,
        summary: "something happened".to_owned(),
        meaning: "something came of it".to_owned(),
        remedies: vec!["do this".to_owned()],
        affected: vec![check.to_owned()],
    }
}

#[test]
fn what_is_owed_survives_the_run_that_owed_it() {
    // The case the outbox was written for and could not do: a channel refuses,
    // the alert is held, and the process ends. Without this it was forgotten.
    let ctx = ctx_at("owed");
    let mut outbox = Outbox::new();
    outbox.owe(vec![alert("service.sonarr")]);
    save(&ctx, &outbox);

    let read_back = load(&ctx);
    assert!(read_back.owes_anything());
    assert_eq!(read_back.owing().len(), 1);
}

#[test]
fn what_was_delivered_stays_in_the_history() {
    // A condition that resolved before anybody read it is still something they
    // are owed the sight of.
    let ctx = ctx_at("history");
    let mut outbox = Outbox::new();
    outbox.owe(vec![alert("service.sonarr")]);
    outbox.delivered(&|_| 0);
    save(&ctx, &outbox);

    assert_eq!(load(&ctx).history().len(), 1);
}

#[test]
fn a_machine_that_has_been_told_nothing_starts_with_nothing() {
    assert!(!load(&ctx_at("fresh")).owes_anything());
}
