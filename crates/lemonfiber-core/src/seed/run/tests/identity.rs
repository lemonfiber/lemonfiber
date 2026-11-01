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

/// The same, with `installed` beside the stack, written to a directory of its own, and the
/// stack's own media server and request service moved into their plugins where each is
/// among `services`.
async fn identity_beside(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    installed: &[crate::plugin::Installed],
    expected: &crate::baseline::Baseline,
) -> (Vec<Wiring>, crate::baseline::Baseline) {
    let (wirings, records, _) = identity_in(ctx, services, installed, expected).await;
    (wirings, records)
}

/// The same, with the stack's directory handed back, for what was written in it.
async fn identity_in(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    installed: &[crate::plugin::Installed],
    expected: &crate::baseline::Baseline,
) -> (
    Vec<Wiring>,
    crate::baseline::Baseline,
    lemonfiber_fixtures::scratch::Scratch,
) {
    static BUILT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nth = BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("identity-stack-{nth}"));
    let key = crate::plugin::key_file(&project, crate::test_support::media::ADAPTER);
    let _ = std::fs::create_dir_all(key.parent().unwrap_or(&project));
    let (wirings, records) = identity_at(ctx, services, installed, expected, &project).await;
    (wirings, records, project)
}

/// The same, with the stack written to `project`, and the plugin a contracted media server
/// comes in held first-party, as the moved request service is.
async fn identity_at(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    installed: &[crate::plugin::Installed],
    expected: &crate::baseline::Baseline,
    project: &std::path::Path,
) -> (Vec<Wiring>, crate::baseline::Baseline) {
    let among = |own: String| services.iter().any(|service| service.id == own);
    let installed: Vec<crate::plugin::Installed> = among(media_server_svc().id)
        .then(crate::test_support::media::moved)
        .flatten()
        .into_iter()
        .chain(
            among(requests_svc().id)
                .then(|| REQUESTS.installed())
                .flatten(),
        )
        .chain(installed.iter().cloned())
        .collect();
    let trusted: Vec<_> = first_party("contracted")
        .into_iter()
        .chain(crate::plugin::first_party::EMBEDDED.iter().copied())
        .collect();
    let fillers = fillers_trusting(services.to_vec(), &installed, project, &trusted);
    let server = crate::app::targets::MediaServer::of(&fillers);
    let admin = super::super::identity::seed_media_server_admin(ctx, server.as_ref()).await;
    super::super::seed_request_identity(ctx, services, &fillers, expected, server.as_ref(), admin)
        .await
}

#[tokio::test]
async fn identity_does_nothing_without_both_the_media_server_and_the_request_service() {
    let ctx = seed_ctx(true, None, None);
    // The request service present but no media server, and the other way round: either
    // alone is nothing to wire.
    let base = crate::baseline::Baseline::new();
    assert!(identity(&ctx, &[requests_svc()], &base).await.0.is_empty());
    assert!(identity(&ctx, &[media_server_svc()], &base)
        .await
        .0
        .is_empty());
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
        crate::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
        "the-stacks-administrator",
    );
    let http = household(true, false);
    let ctx = seed_ctx(true, None, Some(env.to_path_buf())).with_http(http.clone());
    let impostor = crate::test_support::an_installed(
        "impostor",
        vec![crate::test_support::a_placed(
            "seerr",
            &["request.intake"],
            Some(5999),
        )],
    );

    let (wirings, _) = identity_beside(
        &ctx,
        &[media_server_svc()],
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
    // The media server's password was recorded on an earlier run, and both services are
    // already set up: nothing is minted and nothing re-pointed.
    let _ = store::set(
        &env,
        crate::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let ctx = seed_ctx(true, None, Some(env.to_path_buf())).with_http(household(true, true));

    let (wirings, _records) = identity(
        &ctx,
        &[media_server_svc(), requests_svc()],
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

/// A password recorded on an earlier run is handed to the moved server's adapter, which has
/// never held it, and a rehearsal hands it nothing.
#[tokio::test]
async fn a_recorded_password_is_handed_to_the_adapter_and_a_rehearsal_hands_nothing() {
    for rehearsing in [false, true] {
        let env = config_scratch(&format!("jellyfin-handed-{rehearsing}"));
        if let Some(parent) = env.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = store::set(
            &env,
            crate::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
            "minted-earlier",
        );
        let mut ctx =
            seed_ctx(true, None, Some(env.to_path_buf())).with_http(household(true, true));
        ctx.dry_run = rehearsing;

        let (_, _, project) = identity_in(
            &ctx,
            &[media_server_svc(), requests_svc()],
            &[],
            &crate::baseline::Baseline::new(),
        )
        .await;
        let handed = crate::plugin::key_file(&project, crate::test_support::media::ADAPTER)
            .with_file_name(lemonfiber_contract::adapter::UPSTREAM_FILE);
        let held = std::fs::read_to_string(&handed).unwrap_or_default();

        assert_eq!(
            held.contains("minted-earlier"),
            !rehearsing,
            "rehearsing: {rehearsing}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }
}

/// A recorded password the moved server's adapter cannot be handed fails the identity
/// step, naming the server and why.
#[tokio::test]
async fn a_recorded_password_the_adapter_cannot_be_handed_fails_the_identity() {
    let env = config_scratch("jellyfin-unhanded");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = store::set(
        &env,
        crate::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let ctx = seed_ctx(true, None, Some(env.to_path_buf())).with_http(household(true, true));
    let project = lemonfiber_fixtures::scratch::Scratch::new("identity-stack-unhanded");
    let _ = std::fs::create_dir_all(
        crate::plugin::key_file(&project, crate::test_support::media::ADAPTER)
            .with_file_name(lemonfiber_contract::adapter::UPSTREAM_FILE),
    );

    let (wirings, _) = identity_at(
        &ctx,
        &[media_server_svc(), requests_svc()],
        &[],
        &crate::baseline::Baseline::new(),
        &project,
    )
    .await;
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));

    assert!(
        matches!(
            wirings.first().map(|one| &one.state),
            Some(crate::seed::State::Failed { detail })
                if detail.starts_with("Jellyfin could not be handed its administrator's password: ")
        ),
        "{wirings:?}"
    );
}

#[tokio::test]
async fn identity_mints_records_and_wires_a_fresh_household() {
    let env = config_scratch("jellyfin-fresh");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let ctx = seed_ctx(true, Some(vec![0x11; 24]), Some(env.to_path_buf()))
        .with_http(household(false, false));

    let (wirings, records) = identity(
        &ctx,
        &[media_server_svc(), requests_svc()],
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
        Some(crate::seed::said(&crate::seed::wanted_telling()).as_str()),
        "what was written down is not what was written to the service"
    );
    assert!(
        held(&env, "IDENTITY_SOURCE_ADMIN_PASSWORD"),
        "the minted password is recorded"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A fresh household on a machine with nowhere to record the administrator's password
/// is not given one: a password nothing recorded is one lemonfiber is locked out by.
#[tokio::test]
async fn a_fresh_household_with_nowhere_to_record_its_password_is_given_none() {
    let http = household(false, false);
    let ctx = seed_ctx(true, Some(vec![0x11; 24]), None).with_http(http.clone());

    let (wirings, _) = identity(
        &ctx,
        &[media_server_svc(), requests_svc()],
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
        !http.asked_for(crate::test_support::media::identity("create_admin")),
        "the wizard was given a password nothing recorded"
    );
}

/// A password change the media server refuses after the request service's setup is said,
/// with what it leaves open and how to close it, and the minted password stays recorded.
#[tokio::test]
async fn a_password_change_the_media_server_refuses_after_setup_is_said() {
    for (tag, changed) in [
        ("unchanged", crate::test_support::media::refusing()),
        (
            "unadmitted",
            crate::test_support::media::refused(lemonfiber_contract::wire::Kind::Unauthorised),
        ),
    ] {
        a_password_change_refused_after_setup_is_said(tag, changed).await;
    }
}

async fn a_password_change_refused_after_setup_is_said(tag: &str, changed: Answer) {
    let env = config_scratch(&format!("jellyfin-{tag}"));
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let ctx = seed_ctx(true, Some(vec![0x11; 24]), Some(env.to_path_buf()))
        .with_http(super::household_changing(false, false, changed));

    let (wirings, _) = identity(
        &ctx,
        &[media_server_svc(), requests_svc()],
        &crate::baseline::Baseline::new(),
    )
    .await;
    let changed = wirings.get(1);
    let recorded = held(&env, "IDENTITY_SOURCE_ADMIN_PASSWORD");
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
    assert!(
        changed.is_some_and(
            |one| format!("{one:?}").contains("Run lemonfiber credentials rotate jellyfin.")
        ),
        "{changed:?}"
    );
    assert!(recorded, "the administrator's password is recorded");
}

/// A plugin's media server that will not take its changed password is said the same way,
/// and what closes it is the server itself: the operator is pointed at the server and at
/// the setting its own administrator's password is kept under.
#[tokio::test]
async fn a_plugins_server_refusing_its_changed_password_points_at_the_server() {
    use crate::test_support::{a_contracted_media_server, media, CONTRACTED_PORT, MEDIA_ADAPTER};
    let env = config_scratch("jellyfin-plugin-unchanged");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let mut ctx = seed_ctx(true, Some(vec![0x11; 24]), Some(env.to_path_buf()))
        .with_http(super::household_changing(false, false, media::refusing()));
    ctx.seams.engine = Arc::new(
        Reporting::holding(&[MEDIA_ADAPTER], Lifecycle::Running, Health::Healthy)
            .publishing(&[(MEDIA_ADAPTER, "127.0.0.1", CONTRACTED_PORT)])
            .alongside(REQUESTS.running()),
    );
    ctx.seams.confined = media::Keyed::holding(Arc::clone(&ctx.seams.confined), &[MEDIA_ADAPTER]);

    let (wirings, _) = identity_beside(
        &ctx,
        &[requests_svc()],
        &[a_contracted_media_server(Some("jellyfin"))],
        &crate::baseline::Baseline::new(),
    )
    .await;
    let changed = wirings.get(1).map(|one| format!("{one:?}"));
    let recorded = held(&env, "IDENTITY_SOURCE_ADMIN_PASSWORD");
    let plugins = held(&env, "PLUGIN_CONTRACTED_MEDIA__UPSTREAM_ADMIN_PASSWORD");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));

    assert!(
        changed.as_deref().is_some_and(|said| said.contains(
            "lemonfiber config set PLUGIN_CONTRACTED_MEDIA__UPSTREAM_ADMIN_PASSWORD <password>"
        )),
        "{changed:?}"
    );
    assert!(plugins, "the plugin's server's password is recorded");
    assert!(!recorded, "the stack's administrator's password is not");
}

/// What the request service answers holding a telling, on where `enabled` says, for
/// `occasions`.
fn telling(enabled: bool, occasions: &[crate::ports::service::Occasion]) -> Answer {
    crate::test_support::requests::answered(&crate::ports::service::Telling {
        enabled,
        occasions: occasions.iter().copied().collect(),
        others: false,
    })
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
            crate::test_support::media::identity("startup_completed"),
            vec![crate::test_support::media::answered(&true)],
        ),
        (
            crate::test_support::requests::intake("initialized"),
            vec![crate::test_support::requests::answered(&true)],
        ),
        (
            crate::test_support::requests::intake("telling"),
            vec![telling(true, &[crate::ports::service::Occasion::Arrived])],
        ),
    ]);
    let env = config_scratch("telling-theirs");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = store::set(
        &env,
        crate::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let ctx = seed_ctx(true, None, Some(env.to_path_buf())).with_http(http.clone());

    let (wirings, records) = identity(
        &ctx,
        &[media_server_svc(), requests_svc()],
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
            crate::test_support::media::identity("startup_completed"),
            vec![crate::test_support::media::answered(&true)],
        ),
        (
            crate::test_support::requests::intake("initialized"),
            vec![crate::test_support::requests::answered(&true)],
        ),
        (
            crate::test_support::requests::intake("telling"),
            vec![telling(false, &[])],
        ),
    ]);
    let env = config_scratch("telling-off");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = store::set(
        &env,
        crate::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let ctx = seed_ctx(true, None, Some(env.to_path_buf())).with_http(http.clone());

    // lemonfiber recorded that it set the telling on; the service now says off.
    let mut baseline = crate::baseline::Baseline::new();
    baseline.record(
        "seerr",
        crate::seed::TELLING,
        &crate::seed::said(&crate::seed::wanted_telling()),
        "2026-08-28T00:00:00Z",
    );

    let (wirings, records) = identity(&ctx, &[media_server_svc(), requests_svc()], &baseline).await;

    assert_eq!(
        wirings.get(1).map(|wiring| &wiring.state),
        Some(&crate::seed::State::Drifted),
        "their setting was not read as theirs"
    );
    let wrote_to_them = http.requests().iter().any(|asked| {
        asked
            .url
            .ends_with(crate::test_support::requests::intake("tell"))
    });
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

/// **A contracted media server whose upstream names no API is paired with nothing.** The
/// step says which plugin left the API out, and neither the server nor the request
/// service is asked anything.
#[tokio::test]
async fn a_contracted_server_whose_upstream_names_no_api_is_paired_with_nothing() {
    let http = Fake::always(Answer::reply(200, "{}"));
    let ctx = seed_ctx(true, None, None).with_http(http.clone());

    let (wirings, _) = identity_beside(
        &ctx,
        &[requests_svc()],
        &[crate::test_support::a_contracted_media_server(None)],
        &crate::baseline::Baseline::new(),
    )
    .await;

    assert_eq!(wirings.len(), 1, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|one| one.connection == crate::seed::IDENTITY
                && matches!(&one.state, State::Skipped { reason }
                if reason.starts_with("the plugin contracted brings ")
                    && reason.contains("without naming the API its upstream answers in"))),
        "{wirings:?}"
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// **A contracted media server that cannot be asked over `identity.source` is not set
/// up any other way.** The step says so, and nothing is sent to the server, over its
/// contract or through this build's adapter.
#[tokio::test]
async fn a_contracted_server_unanswered_over_its_contract_is_not_set_up() {
    let http = Fake::always(Answer::reply(200, "{}"));
    let ctx = seed_ctx(true, None, None).with_http(http.clone());

    let (wirings, _) = identity_beside(
        &ctx,
        &[requests_svc()],
        &[crate::test_support::a_contracted_media_server(Some(
            "upstream",
        ))],
        &crate::baseline::Baseline::new(),
    )
    .await;

    assert!(
        wirings
            .iter()
            .any(|one| one.connection == crate::seed::IDENTITY
                && matches!(&one.state, State::Skipped { reason }
                if reason.ends_with(
                    " speaks the identity source's contract and cannot be asked over it"
                ))),
        "{wirings:?}"
    );
    let asked = http.requests();
    assert!(
        !asked
            .iter()
            .any(|one| one.url.contains(":8080") || one.url.contains(":9000")),
        "{asked:?}"
    );
}

/// Whether the secret store beside `env` holds a credential under `name`.
fn held(env: &std::path::Path, name: &str) -> bool {
    crate::settings::Secrets::at(&env.with_file_name(crate::config::paths::SECRETS))
        .is_ok_and(|kept| kept.value(name).is_some())
}
