use crate::app::Ctx;
use crate::config::{Protocols, Settings};
use crate::model::{BootOutcome, BootReport, BootTunnel};
use crate::test_support::spoke;

use super::super::tests::{
    asked, asked_for_the_library, awake, ctx_answering, ctx_at, ctx_coming_back, never_there,
    plugged_in, run, settings_at, Machine,
};
use super::booted;

/// What the read answers for `ctx`, or why it refused.
async fn read(ctx: &Ctx) -> Result<BootReport, String> {
    booted(ctx).await.map_err(|problem| problem.summary)
}

/// The same records, read on a machine that started at `at`, or will not say.
fn restarted(settings: Settings, at: Option<u64>) -> Ctx {
    let machine = Machine {
        at,
        power: plugged_in(0).power,
    };
    ctx_answering(settings, machine, awake(), spoke(""))
}

#[tokio::test]
async fn a_stack_that_came_back_is_read_as_passed_against_the_restart_it_followed() {
    let ctx = ctx_coming_back(settings_at("kept-came-back"));
    asked_for_the_library(&ctx);
    let _ = run(&ctx).await;

    let report = read(&ctx).await.unwrap_or_default();

    assert_eq!(report.booted_at, Some(1_000));
    let verified = report.verified.clone();
    assert_eq!(
        verified
            .as_ref()
            .map(|one| (one.against, one.outcome, one.tunnel.clone())),
        Some((Some(1_000), BootOutcome::Passed, BootTunnel::NotChecked)),
        "{report:?}"
    );
    assert!(verified
        .as_ref()
        .is_some_and(|one| one.not_back.is_empty() && one.said.is_none()));
    assert_eq!(report.last_verified, verified);
    assert_eq!(
        (report.declined, report.failing_since, report.failed_boots),
        (None, None, None)
    );
}

#[tokio::test]
async fn a_stack_back_without_its_tunnel_is_read_as_failed_with_the_condition_counted() {
    let ctx = ctx_coming_back(Settings {
        protocols: Protocols::both(),
        ..settings_at("kept-no-tunnel")
    });
    asked_for_the_library(&ctx);
    let _ = run(&ctx).await;

    let report = read(&ctx).await.unwrap_or_default();

    let verified = report.verified.as_ref();
    assert_eq!(verified.map(|one| one.outcome), Some(BootOutcome::Failed));
    assert!(
        verified.is_some_and(|one| matches!(
            &one.tunnel,
            BootTunnel::NotEstablished { said } if said.contains("its tunnel did not")
        ) && one
            .said
            .as_deref()
            .is_some_and(|said| said.contains("its tunnel did not"))),
        "{report:?}"
    );
    assert_eq!(report.failed_boots, Some(0));
    assert!(report.failing_since.is_some(), "{report:?}");
}

#[tokio::test]
async fn a_start_compose_refused_is_read_as_failed_with_what_it_said() {
    let ctx = ctx_at("kept-refused", plugged_in(1_000), awake());
    asked(
        &ctx,
        &crate::autostart::Returning::default().answering(true),
    );
    let _ = run(&ctx).await;

    let report = read(&ctx).await.unwrap_or_default();

    let verified = report.verified.as_ref();
    assert_eq!(verified.map(|one| one.outcome), Some(BootOutcome::Failed));
    assert!(verified.is_some_and(|one| one.said.is_some()), "{report:?}");
    assert!(report.failing_since.is_some());
}

#[tokio::test]
async fn a_run_that_started_nothing_is_read_with_why_and_no_verification() {
    let declined = ctx_at("kept-not-asked", plugged_in(1_000), awake());
    let _ = run(&declined).await;
    let report = read(&declined).await.unwrap_or_default();
    assert!(report.verified.is_none() && report.last_verified.is_none());
    assert!(
        report
            .declined
            .is_some_and(|said| said.contains("not asked to start")),
        "a decline is kept against the restart it followed"
    );

    let absent = ctx_at("kept-no-engine", plugged_in(1_000), never_there());
    asked(
        &absent,
        &crate::autostart::Returning::default().answering(true),
    );
    let _ = run(&absent).await;
    assert_eq!(
        read(&absent).await.ok().and_then(|report| report.declined),
        Some(super::super::ENGINE_NEVER_CAME.to_owned())
    );
}

#[tokio::test]
async fn a_verification_after_an_earlier_restart_is_never_read_as_the_latest() {
    let settings = settings_at("kept-earlier");
    let ctx = ctx_coming_back(settings.clone());
    asked_for_the_library(&ctx);
    let _ = run(&ctx).await;

    for at in [Some(2_000), None] {
        let report = read(&restarted(settings.clone(), at))
            .await
            .unwrap_or_default();
        assert_eq!(report.booted_at, at);
        assert_eq!(report.verified, None, "{at:?}");
        assert_eq!(
            report.last_verified.map(|one| one.against),
            Some(Some(1_000)),
            "{at:?}"
        );
    }
}

#[tokio::test]
async fn a_rehearsed_boot_keeps_nothing_to_read() {
    let ctx = ctx_coming_back(settings_at("kept-rehearsed")).rehearsing();
    asked_for_the_library(&ctx);
    let _ = run(&ctx).await;

    let report = read(&ctx).await.unwrap_or_default();

    assert_eq!(
        (report.verified, report.last_verified, report.declined),
        (None, None, None)
    );
}

#[tokio::test]
async fn a_record_that_cannot_be_read_is_refused_naming_it_rather_than_read_as_none() {
    for (name, record) in [
        ("kept-damaged", crate::config::paths::BOOT),
        ("kept-damaged-conditions", crate::config::paths::CONDITIONS),
    ] {
        let ctx = ctx_at(name, plugged_in(1_000), awake());
        let path = crate::app::targets::beside_env(&ctx, record);
        if let Some(path) = &path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, "{ not json");
        }

        let refused = read(&ctx).await.err();

        let named = path.map(|path| path.display().to_string());
        assert!(
            refused.is_some_and(|said| named.is_some_and(|named| said.contains(&named))),
            "{name}"
        );
    }
}
