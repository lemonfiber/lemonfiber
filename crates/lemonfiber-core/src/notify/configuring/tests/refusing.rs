//! What every request answers where the back-ends cannot be kept, read or written, what a
//! rehearsal of each comes to, and what a request prints of itself.

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{add, code, file, machine, ntfy, quiet, run, TOKEN};
use crate::app::{Ctx, Outcome};
use crate::error::codes::notify::{NOT_A_RETRY_SCHEDULE, NO_SUCH_BACKEND};
use crate::model::BackendProof;
use crate::notify::backends;
use crate::notify::configuring::{asked, Asked, Setting};

const REKEYED: &str = "tk_rekeyed";

fn rekeying(id: &str) -> Asked {
    Asked::Set(Box::new(Setting {
        id: id.to_owned(),
        token: Some(REKEYED.to_owned()),
        ..Setting::default()
    }))
}

fn testing(id: &str) -> Asked {
    Asked::Test { id: id.to_owned() }
}

fn removing(id: &str) -> Asked {
    Asked::Remove { id: id.to_owned() }
}

fn choosing_nothing() -> Asked {
    Asked::Choose {
        member: "ana".to_owned(),
        backends: Vec::new(),
    }
}

fn every(id: &str) -> Vec<Asked> {
    vec![
        Asked::List,
        Asked::Add(Box::new(ntfy(id))),
        rekeying(id),
        testing(id),
        removing(id),
        Asked::Mine {
            member: "ana".to_owned(),
        },
        choosing_nothing(),
    ]
}

async fn refused(ctx: &Ctx, what: Asked) -> bool {
    asked(ctx, what).await.is_err()
}

fn record(ctx: &Ctx) -> std::path::PathBuf {
    backends::at(ctx).unwrap_or_default()
}

fn secrets(ctx: &Ctx) -> std::path::PathBuf {
    backends::secrets_at(ctx).unwrap_or_default()
}

#[tokio::test]
async fn a_machine_keeping_no_configuration_keeps_no_back_end() {
    let ctx = crate::test_support::a_context().build();

    for what in every("home-ntfy") {
        assert!(refused(&ctx, what).await);
    }
}

#[tokio::test]
async fn a_record_that_cannot_be_read_refuses_every_request_rehearsed_or_not() {
    let ctx = quiet("notify-unreadable");
    assert!(std::fs::write(record(&ctx), "{ not a record").is_ok());

    for ctx in [ctx.clone(), ctx.clone().rehearsing()] {
        for what in every("home-ntfy") {
            assert!(refused(&ctx, what).await);
        }
    }
    assert_eq!(file(backends::at(&ctx)), "{ not a record");
    assert!(!secrets(&ctx).exists());
}

#[tokio::test]
async fn a_secret_that_cannot_be_written_refuses_the_request_that_wrote_it() {
    let ctx = quiet("notify-unwritten-secret");
    assert!(add(&ctx, ntfy("home-ntfy")).await.is_ok());
    let kept = file(backends::secrets_at(&ctx));
    let blocked = crate::test_support::blocking(&secrets(&ctx));

    let added = add(&ctx, ntfy("away-ntfy")).await;
    let set = run(&ctx, rekeying("home-ntfy")).await;
    let removed = run(&ctx, removing("home-ntfy")).await;

    assert!(added.is_err() && set.is_err() && removed.is_err());
    assert_eq!(file(backends::secrets_at(&ctx)), kept);
    assert!(std::fs::remove_dir_all(blocked).is_ok());
}

#[tokio::test]
async fn a_proof_that_cannot_be_kept_is_refused() {
    let ctx = machine(
        "notify-unkept-proof",
        Fake::always(Answer::reply(403, "{}")),
    );
    let _ = add(&ctx, ntfy("home-ntfy")).await;
    let blocked = crate::test_support::blocking(&record(&ctx));

    let tested = run(&ctx, testing("home-ntfy")).await;

    assert!(tested.is_err());
    assert!(std::fs::remove_dir_all(blocked).is_ok());
    assert_eq!(
        run(&ctx, Asked::List)
            .await
            .ok()
            .and_then(|report| report.backends.into_iter().next())
            .map(|one| one.proof),
        Some(BackendProof::Unproven)
    );
}

#[tokio::test]
async fn a_rehearsed_change_test_or_removal_is_checked_and_keeps_nothing() {
    let ctx = quiet("notify-rehearsed-each");
    let _ = add(&ctx, ntfy("home-ntfy")).await;
    let before = file(backends::at(&ctx));
    let rehearsing = ctx.clone().rehearsing();

    for what in [
        rekeying("home-ntfy"),
        testing("home-ntfy"),
        removing("home-ntfy"),
    ] {
        let report = run(&rehearsing, what).await;
        assert_eq!(report.map(|report| report.rehearsed).ok(), Some(true));
    }
    let unknown = run(&rehearsing, removing("nowhere")).await;
    let unscheduled = run(
        &rehearsing,
        Asked::Set(Box::new(Setting {
            id: "home-ntfy".to_owned(),
            attempts: Some(4),
            ..Setting::default()
        })),
    )
    .await;

    assert_eq!(code(&unknown), Some(NO_SUCH_BACKEND));
    assert_eq!(code(&unscheduled), Some(NOT_A_RETRY_SCHEDULE));
    assert_eq!(file(backends::at(&ctx)), before);
    assert!(file(backends::secrets_at(&ctx)).contains(TOKEN));
}

#[tokio::test]
async fn attempts_alone_keep_the_delays_and_must_agree_with_them() {
    let ctx = quiet("notify-attempts");
    let _ = add(&ctx, ntfy("home-ntfy")).await;
    let schedule = || {
        backends::kept(&record(&ctx))
            .ok()
            .and_then(|kept| kept.backends.into_iter().next())
            .map(|one| (one.attempts, one.delays))
    };
    let attempts = |attempts| {
        Asked::Set(Box::new(Setting {
            id: "home-ntfy".to_owned(),
            attempts: Some(attempts),
            ..Setting::default()
        }))
    };
    let before = schedule();

    let kept = run(&ctx, attempts(backends::ATTEMPTS)).await;
    let disagreeing = run(&ctx, attempts(2)).await;

    assert!(kept.is_ok());
    assert_eq!(code(&disagreeing), Some(NOT_A_RETRY_SCHEDULE));
    assert!(before.is_some());
    assert_eq!(schedule(), before);
}

#[tokio::test]
async fn removing_one_of_two_keeps_the_other_and_what_was_told() {
    let ctx = quiet("notify-one-of-two");
    let _ = add(&ctx, ntfy("home-ntfy")).await;
    let _ = add(&ctx, ntfy("away-ntfy")).await;
    let told = backends::changed(&record(&ctx), &mut |kept: &mut backends::Kept| {
        kept.told = Some(vec!["service.jellyfin".to_owned()]);
        Ok(())
    });
    assert!(told.is_ok());

    let removed = run(&ctx, removing("home-ntfy")).await;

    let left = removed.ok().map(|report| {
        report
            .backends
            .into_iter()
            .map(|one| one.id)
            .collect::<Vec<_>>()
    });
    assert_eq!(left, Some(vec!["away-ntfy".to_owned()]));
    assert_eq!(
        backends::kept(&record(&ctx))
            .ok()
            .and_then(|kept| kept.told),
        Some(vec!["service.jellyfin".to_owned()])
    );
}

#[tokio::test]
async fn a_member_choosing_is_answered_with_what_they_are_offered() {
    let ctx = quiet("notify-choosing-asked");

    let chosen = asked(&ctx, choosing_nothing()).await;

    assert!(matches!(chosen, Ok(Outcome::NotifyMine(report)) if report.offered.is_empty()));
}

#[test]
fn what_was_asked_prints_its_names_and_never_a_secret() {
    for what in every("home-ntfy") {
        let printed = format!("{what:?}");
        assert!(!printed.contains(TOKEN) && !printed.contains(REKEYED));
        assert!(["List", "home-ntfy", "ana"]
            .iter()
            .any(|named| printed.contains(named)));
    }
}
