//! The household's identity source, and what the operator switched on or off.

use super::*;

#[tokio::test]
async fn identity_does_nothing_without_both_jellyfin_and_seerr() {
    let ctx = seed_ctx(None, true, Vec::new(), None, None);
    // Seerr present but no Jellyfin, and the other way round: either alone is
    // nothing to wire.
    let base = crate::baseline::Baseline::new();
    assert!(
        super::super::seed_jellyfin_identity(&ctx, &[seerr_svc()], &base, &identified())
            .await
            .0
            .is_empty()
    );
    assert!(
        super::super::seed_jellyfin_identity(&ctx, &[jellyfin_svc()], &base, &identified())
            .await
            .0
            .is_empty()
    );
}

#[tokio::test]
async fn identity_leaves_an_already_set_up_household_alone() {
    let env = config_scratch("jellyfin-already");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Jellyfin's password was recorded on an earlier run, and both services are
    // already set up: nothing is minted and nothing re-pointed.
    let _ = store::set(
        &env,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let ctx =
        seed_ctx(None, true, Vec::new(), None, Some(env.clone())).with_http(household(true, true));

    let (wirings, _records) = super::super::seed_jellyfin_identity(
        &ctx,
        &[jellyfin_svc(), seerr_svc()],
        &crate::baseline::Baseline::new(),
        &identified(),
    )
    .await;
    assert_eq!(wirings.len(), 2);
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::AlreadyWired)
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn identity_mints_records_and_wires_a_fresh_household() {
    let env = config_scratch("jellyfin-fresh");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let ctx = seed_ctx(
        None,
        true,
        Vec::new(),
        Some(vec![0x11; 24]),
        Some(env.clone()),
    )
    .with_http(household(false, false));

    let (wirings, records) = super::super::seed_jellyfin_identity(
        &ctx,
        &[jellyfin_svc(), seerr_svc()],
        &crate::baseline::Baseline::new(),
        &identified(),
    )
    .await;
    assert_eq!(wirings.len(), 2);
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "a fresh household is minted, signed in, and confirmed"
    );
    // A service nobody has configured is one nobody in the house hears from, so
    // the telling is written rather than left at the untouched default — and the
    // baseline records what was written, or the next run reads this as the
    // operator's own value and preserves an absence.
    assert_eq!(
        wirings.get(1).map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "a household that has never been told anything is set up to be told"
    );
    assert_eq!(
        records
            .entry("seerr", crate::seed::TELLING)
            .map(|record| record.value.as_str()),
        Some(crate::seed::said(crate::seed::wanted_telling()).as_str()),
        "what was written down is not what was written to the service"
    );
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        written.contains("JELLYFIN_ADMIN_PASSWORD="),
        "the minted password is recorded: {written}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A telling the operator set before lemonfiber ever ran is taken on, not flagged.
///
/// The baseline is empty and the service holds something, so there is no
/// expectation to have drifted from — adopting it is what stops an existing setup
/// being reported as wholesale drift on the first pass.
#[tokio::test]
async fn a_telling_set_before_lemonfiber_ran_is_adopted_as_the_baseline() {
    let http = Fake::by_path_in_turn(vec![
        (
            "/System/Info/Public",
            vec![Answer::reply(200, r#"{"StartupWizardCompleted":true}"#)],
        ),
        (
            "/settings/notifications/webpush",
            vec![Answer::reply(200, r#"{"enabled":true,"types":8}"#)],
        ),
        ("", vec![Answer::reply(200, r#"{"initialized":true}"#)]),
    ]);
    let env = config_scratch("telling-theirs");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = store::set(
        &env,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone())).with_http(http.clone());

    let (wirings, records) = super::super::seed_jellyfin_identity(
        &ctx,
        &[jellyfin_svc(), seerr_svc()],
        &crate::baseline::Baseline::new(),
        &identified(),
    )
    .await;

    assert_eq!(
        wirings.get(1).map(|wiring| &wiring.state),
        Some(&crate::seed::State::Unmanaged),
        "a pre-existing value was not read as the operator's own"
    );
    let taken = records.entry("seerr", crate::seed::TELLING);
    assert!(
        taken.is_some_and(|record| record.origin.is_adopted()),
        "their value was not adopted as the baseline, so the next pass reports it \
         as drift from an expectation nobody formed"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// An operator who switched it off switched it off.
///
/// The case the three-way comparison exists for. Two values could only say the
/// service differs from what lemonfiber wants, and reverting on that would take
/// back a decision somebody made on purpose. The third — what lemonfiber last
/// recorded — is what says the change was theirs. Asserted by what the service
/// was asked to do, not only by the state reported: a pass that wrote over them
/// and then called it drift would satisfy the state alone.
#[tokio::test]
async fn a_telling_the_operator_switched_off_is_reported_rather_than_overruled() {
    let http = Fake::by_path_in_turn(vec![
        (
            "/System/Info/Public",
            vec![Answer::reply(200, r#"{"StartupWizardCompleted":true}"#)],
        ),
        (
            "/settings/notifications/webpush",
            vec![Answer::reply(200, r#"{"enabled":false,"types":0}"#)],
        ),
        ("", vec![Answer::reply(200, r#"{"initialized":true}"#)]),
    ]);
    let env = config_scratch("telling-off");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = store::set(
        &env,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone())).with_http(http.clone());

    // lemonfiber recorded that it set the telling on; the service now says off.
    let mut baseline = crate::baseline::Baseline::new();
    baseline.record(
        "seerr",
        crate::seed::TELLING,
        &crate::seed::said(crate::seed::wanted_telling()),
        "2026-08-28T00:00:00Z",
    );

    let (wirings, records) = super::super::seed_jellyfin_identity(
        &ctx,
        &[jellyfin_svc(), seerr_svc()],
        &baseline,
        &identified(),
    )
    .await;

    assert_eq!(
        wirings.get(1).map(|wiring| &wiring.state),
        Some(&crate::seed::State::Drifted),
        "their setting was not read as theirs"
    );
    let wrote_to_them = http
        .requests()
        .iter()
        .any(|asked| asked.url.contains("notifications/webpush") && asked.method == Method::Post);
    assert!(
        !wrote_to_them,
        "the service was written to, so their setting was overruled"
    );
    assert!(
        records.entry("seerr", crate::seed::TELLING).is_none(),
        "a preserved edit re-recorded lemonfiber's own value, so the next run \
         would read it as agreement and stop reporting it"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}
