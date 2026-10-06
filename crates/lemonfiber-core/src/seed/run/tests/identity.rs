//! The household's identity source, and what the operator switched on or off.

use super::*;

/// Both halves of the identity step, as a run takes them on a stack without the gate.
async fn identity(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    expected: &crate::baseline::Baseline,
) -> (Vec<Wiring>, crate::baseline::Baseline) {
    identity_beside(ctx, services, &[], expected).await
}

/// The same, with `installed` beside the stack.
async fn identity_beside(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    installed: &[crate::plugin::Installed],
    expected: &crate::baseline::Baseline,
) -> (Vec<Wiring>, crate::baseline::Baseline) {
    let fillers = fillers_beside(services.to_vec(), installed, stack_root());
    let server = crate::app::targets::MediaServer::of(&fillers);
    let admin = super::super::identity::seed_jellyfin_admin(ctx, server.as_ref()).await;
    super::super::seed_jellyfin_identity(ctx, services, expected, server.as_ref(), admin, None)
        .await
}

#[tokio::test]
async fn identity_does_nothing_without_both_jellyfin_and_seerr() {
    let ctx = seed_ctx(None, true, Vec::new(), None, None);
    // Seerr present but no Jellyfin, and the other way round: either alone is
    // nothing to wire.
    let base = crate::baseline::Baseline::new();
    assert!(identity(&ctx, &[seerr_svc()], &base).await.0.is_empty());
    assert!(identity(&ctx, &[jellyfin_svc()], &base).await.0.is_empty());
}

/// **The stack's administrator's password is never sent to a plugin.** A plugin's
/// service running under the id the stack's request service asks under, on a stack whose
/// own request service is gone, is never signed in to the stack's media server with it,
/// and is sent nothing at all.
#[tokio::test]
async fn a_plugin_under_the_request_services_id_is_never_sent_the_administrators_password() {
    let env = config_scratch("jellyfin-impostor");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = store::set(
        &env,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "the-stacks-administrator",
    );
    let http = household(true, false);
    let ctx =
        seed_ctx(None, true, Vec::new(), None, Some(env.to_path_buf())).with_http(http.clone());
    let impostor = crate::test_support::an_installed(
        "impostor",
        vec![crate::test_support::a_placed(
            "seerr",
            &["request.intake"],
            Some(seerr_api()),
            Some(5999),
        )],
    );

    let (wirings, _) = identity_beside(
        &ctx,
        &[jellyfin_svc()],
        &[impostor],
        &crate::baseline::Baseline::new(),
    )
    .await;

    let sent = http.requests();
    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(
        !sent.iter().any(|asked| asked.url.contains(":5999")),
        "{sent:?}"
    );
    assert!(
        !sent.iter().any(|asked| !asked.url.contains(":8096")
            && asked
                .body
                .as_deref()
                .is_some_and(|body| body.contains("the-stacks-administrator"))),
        "{sent:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
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
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.to_path_buf()))
        .with_http(household(true, true));

    let (wirings, _records) = identity(
        &ctx,
        &[jellyfin_svc(), seerr_svc()],
        &crate::baseline::Baseline::new(),
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
        Some(env.to_path_buf()),
    )
    .with_http(household(false, false));

    let (wirings, records) = identity(
        &ctx,
        &[jellyfin_svc(), seerr_svc()],
        &crate::baseline::Baseline::new(),
    )
    .await;
    assert_eq!(wirings.len(), 3);
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "a fresh household is minted, signed in, and confirmed"
    );
    // The run that showed the request service the administrator's password changes it.
    assert_eq!(
        wirings
            .get(1)
            .map(|wiring| (wiring.connection.as_str(), &wiring.state)),
        Some((
            "Jellyfin's administrator password, changed once the request service was set up",
            &crate::seed::State::Wired
        ))
    );
    // A service nobody has configured is one nobody in the house hears from, so
    // the telling is written rather than left at the untouched default — and the
    // baseline records what was written, or the next run reads this as the
    // operator's own value and preserves an absence.
    assert_eq!(
        wirings.get(2).map(|wiring| &wiring.state),
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

/// A fresh household on a machine with nowhere to record the administrator's password
/// is not given one: a password nothing recorded is one lemonfiber is locked out by.
#[tokio::test]
async fn a_fresh_household_with_nowhere_to_record_its_password_is_given_none() {
    let http = household(false, false);
    let ctx = seed_ctx(None, true, Vec::new(), Some(vec![0x11; 24]), None).with_http(http.clone());

    let (wirings, _) = identity(
        &ctx,
        &[jellyfin_svc(), seerr_svc()],
        &crate::baseline::Baseline::new(),
    )
    .await;

    assert!(
        wirings.first().is_some_and(|wiring| matches!(
            &wiring.state,
            crate::seed::State::Failed { detail } if detail.contains("could not be recorded")
        )),
        "the administrator connection did not fail over the record"
    );
    assert!(
        !http.asked_for("/Startup/User"),
        "the wizard was given a password nothing recorded"
    );
}

/// A password change Jellyfin refuses after the request service's setup is said, with
/// what it leaves open and how to close it, and the minted password stays recorded.
#[tokio::test]
async fn a_password_change_jellyfin_refuses_after_setup_is_said() {
    for (tag, admitted, changed) in [("unchanged", 200, 500), ("unadmitted", 401, 204)] {
        a_password_change_refused_after_setup_is_said(tag, admitted, changed).await;
    }
}

async fn a_password_change_refused_after_setup_is_said(tag: &str, admitted: u16, changed: u16) {
    let env = config_scratch(&format!("jellyfin-{tag}"));
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let ctx = seed_ctx(
        None,
        true,
        Vec::new(),
        Some(vec![0x11; 24]),
        Some(env.to_path_buf()),
    )
    .with_http(super::household_changing(false, false, admitted, changed));

    let (wirings, _) = identity(
        &ctx,
        &[jellyfin_svc(), seerr_svc()],
        &crate::baseline::Baseline::new(),
    )
    .await;
    let changed = wirings.get(1);
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));

    assert!(
        matches!(
            changed.map(|one| &one.state),
            Some(crate::seed::State::Failed { .. })
        ),
        "{changed:?}"
    );
    assert!(
        changed.is_some_and(|one| format!("{one:?}").contains("still opens Jellyfin")),
        "{changed:?}"
    );
    assert!(written.contains("JELLYFIN_ADMIN_PASSWORD="), "{written}");
}

/// A plugin's media server that will not take its changed password is said the same way,
/// and what closes it is the server itself: lemonfiber does not rotate a plugin's
/// credentials, so the operator is pointed at the server and at the setting its own
/// administrator's password is kept under.
#[tokio::test]
async fn a_plugins_server_refusing_its_changed_password_points_at_the_server() {
    let env = config_scratch("jellyfin-plugin-unchanged");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let ctx = seed_ctx(
        None,
        true,
        Vec::new(),
        Some(vec![0x11; 24]),
        Some(env.to_path_buf()),
    )
    .with_http(super::household_changing(false, false, 200, 500));
    let mut placed = crate::test_support::a_placed(
        "emby",
        &["identity.source"],
        Some(jellyfin_api()),
        Some(8920),
    );
    placed.tag = "10.10.7".to_owned();
    let server = crate::test_support::an_installed("emby-server", vec![placed]);

    let (wirings, _) = identity_beside(
        &ctx,
        &[seerr_svc()],
        &[server],
        &crate::baseline::Baseline::new(),
    )
    .await;
    let changed = wirings.get(1).map(|one| format!("{one:?}"));
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));

    assert!(
        changed.as_deref().is_some_and(|said| said.contains(
            "`lemonfiber config set PLUGIN_EMBY__SERVER_EMBY_ADMIN_PASSWORD <password>`"
        )),
        "{changed:?}"
    );
    assert!(
        written.contains("PLUGIN_EMBY__SERVER_EMBY_ADMIN_PASSWORD="),
        "{written}"
    );
    assert!(!written.contains("JELLYFIN_ADMIN_PASSWORD="), "{written}");
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
    let ctx =
        seed_ctx(None, true, Vec::new(), None, Some(env.to_path_buf())).with_http(http.clone());

    let (wirings, records) = identity(
        &ctx,
        &[jellyfin_svc(), seerr_svc()],
        &crate::baseline::Baseline::new(),
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
    let ctx =
        seed_ctx(None, true, Vec::new(), None, Some(env.to_path_buf())).with_http(http.clone());

    // lemonfiber recorded that it set the telling on; the service now says off.
    let mut baseline = crate::baseline::Baseline::new();
    baseline.record(
        "seerr",
        crate::seed::TELLING,
        &crate::seed::said(crate::seed::wanted_telling()),
        "2026-08-28T00:00:00Z",
    );

    let (wirings, records) = identity(&ctx, &[jellyfin_svc(), seerr_svc()], &baseline).await;

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
