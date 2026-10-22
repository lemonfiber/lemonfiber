//! A request service speaking `request.intake`: set up, told and read over the contract.

use super::*;
use crate::test_support::{CONTRACTED_REQUESTS, CONTRACTED_REQUESTS_AT};

/// A plugin `intake` whose service, under [`CONTRACTED_REQUESTS`], fills
/// `request.intake` over the contract and is what asks for the identity source, beside
/// the stack's media server moved into its plugin, with `trusted` and every plugin this
/// build embeds as the first-party plugins.
fn beside_contracted_requests(
    project: &std::path::Path,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    let mut intake = contracted("intake", CONTRACTED_REQUESTS, "request.intake");
    for placed in &mut intake.services {
        placed.asks = vec![crate::plugin::Asking {
            capability: crate::app::targets::IDENTITY.to_owned(),
            each: false,
        }];
    }
    crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            manifest.services = vec![media_server_svc()];
            manifest
                .wirings
                .retain(|wiring| wiring.asks.as_deref() != Some(crate::app::targets::IDENTITY));
            let installed: Vec<_> = crate::test_support::media::moved()
                .into_iter()
                .chain([intake])
                .collect();
            let trusted: Vec<_> = trusted
                .iter()
                .copied()
                .chain(crate::plugin::first_party::EMBEDDED.iter().copied())
                .collect();
            crate::wiring::Fillers::trusting(
                &manifest,
                &installed,
                &crate::wiring::Chosen::default(),
                Some(project),
                &trusted,
            )
        })
        .unwrap_or_default()
}

/// A context over `project` reaching the contracted request service where `keyed`, with
/// the media server's administrator's password recorded in `env` and every call answered
/// by `http`.
fn contracted_identity_ctx(
    project: &std::path::Path,
    env: &std::path::Path,
    keyed: bool,
    http: Arc<Fake>,
) -> Ctx {
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = store::set(
        env,
        crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
        ADMINISTRATOR,
    );
    let mut ctx = contracted_ctx(project, CONTRACTED_REQUESTS, keyed, http)
        .with_random(Arc::new(FixedRandom(Some(vec![0x11; 24]))));
    ctx.settings.env_file = Some(env.to_path_buf());
    ctx.seams.confined = crate::test_support::media::Keyed::over(Arc::clone(&ctx.seams.confined));
    ctx
}

/// The media server's administrator's password a contracted test records.
const ADMINISTRATOR: &str = "the-stacks-administrator";

/// Both halves of the identity step over `fillers`, on a stack without the gate.
async fn identity_over(
    ctx: &Ctx,
    fillers: &crate::wiring::Fillers,
) -> (Vec<Wiring>, crate::baseline::Baseline) {
    let server = crate::app::targets::MediaServer::of(fillers);
    let admin = super::super::identity::seed_media_server_admin(ctx, server.as_ref()).await;
    super::super::seed_request_identity(
        ctx,
        &[media_server_svc()],
        fillers,
        &crate::baseline::Baseline::new(),
        server.as_ref(),
        admin,
        None,
    )
    .await
}

/// **A first-party request service speaking `request.intake` is set up and told over
/// it.** It is signed in through the media server over the contract, what the household
/// is told is written over the contract and recorded under its own id, and the bundled
/// request service's API is never asked.
#[tokio::test]
async fn a_first_party_request_service_is_set_up_and_told_over_the_contract() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("identity-contracted");
    let env = config_scratch("identity-contracted");
    let http = Fake::by_path_in_turn(vec![
        (
            crate::test_support::media::identity("startup_completed"),
            vec![crate::test_support::media::answered(&true)],
        ),
        (
            crate::test_support::media::identity("replace_admin_password"),
            vec![crate::test_support::media::taken()],
        ),
        (
            "/v1/initialized",
            vec![Answer::reply(200, "false"), Answer::reply(200, "true")],
        ),
        ("/v1/configure_identity", vec![Answer::reply(204, "")]),
        (
            "/v1/telling",
            vec![Answer::reply(
                200,
                r#"{"enabled":false,"occasions":[],"others":false}"#,
            )],
        ),
        ("/v1/tell", vec![Answer::reply(204, "")]),
    ]);
    let ctx = contracted_identity_ctx(&project, &env, true, http.clone());

    let (wirings, records) = identity_over(
        &ctx,
        &beside_contracted_requests(&project, &first_party("intake")),
    )
    .await;

    let states: Vec<&State> = wirings.iter().map(|wiring| &wiring.state).collect();
    assert_eq!(
        states,
        vec![&State::Wired, &State::Wired, &State::Wired],
        "{wirings:?}"
    );
    let asked = http.requests();
    let at = |op: &str| format!("{CONTRACTED_REQUESTS_AT}{op}");
    assert!(
        asked.iter().any(|one| one.url == at("configure_identity")
            && one
                .body
                .as_deref()
                .is_some_and(|body| body.contains(ADMINISTRATOR))),
        "{asked:?}"
    );
    assert!(asked.iter().any(|one| one.url == at("tell")), "{asked:?}");
    assert!(
        !asked.iter().any(|one| one.url.contains("/api/v1/")),
        "{asked:?}"
    );
    assert!(records
        .entry(CONTRACTED_REQUESTS, crate::seed::TELLING)
        .is_some());
    assert!(records.entry("seerr", crate::seed::TELLING).is_none());
}

/// **A request service speaking `request.intake` that cannot be asked over it, or that
/// the trust gate keeps the administrator's password from, is asked nothing.** Neither a
/// first-party one holding no key nor a third-party one is set up, told or sent the
/// administrator's password, over the contract or any other way.
#[tokio::test]
async fn a_contracted_request_service_unreachable_or_untrusted_is_asked_nothing() {
    for (tag, trusted, keyed) in [
        ("unkeyed", &first_party("intake")[..], false),
        ("untrusted", &[][..], true),
    ] {
        let project =
            lemonfiber_fixtures::scratch::Scratch::new(&format!("identity-contracted-{tag}"));
        let env = config_scratch(&format!("identity-contracted-{tag}"));
        let http = household(true, false);
        let ctx = contracted_identity_ctx(&project, &env, keyed, http.clone());

        let fillers = beside_contracted_requests(&project, trusted);
        assert_eq!(
            crate::app::targets::MediaServer::of(&fillers)
                .as_ref()
                .and_then(crate::test_support::asker),
            (!trusted.is_empty()).then_some(CONTRACTED_REQUESTS),
            "{tag}"
        );

        let (wirings, records) = identity_over(&ctx, &fillers).await;

        let asked = http.requests();
        assert!(wirings.is_empty(), "{tag}: {wirings:?}");
        assert!(
            records
                .entry(CONTRACTED_REQUESTS, crate::seed::TELLING)
                .is_none(),
            "{tag}"
        );
        assert!(
            !asked.iter().any(|one| one.url.contains("/request.intake/")
                || one.url.contains("/api/v1/")
                || (!one.url.contains(":8096")
                    && one
                        .body
                        .as_deref()
                        .is_some_and(|body| body.contains(ADMINISTRATOR)))),
            "{tag}: {asked:?}"
        );
    }
}

/// **What the household is told is read over `request.intake` where the request service
/// speaks it**, against what lemonfiber recorded under that service's own id.
#[tokio::test]
async fn the_household_telling_is_read_over_the_contract_against_its_own_record() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("telling-contracted");
    let env = config_scratch("telling-contracted");
    let http = Fake::by_path_in_turn(vec![(
        "/v1/telling",
        vec![Answer::reply(
            200,
            r#"{"enabled":true,"occasions":[],"others":false}"#,
        )],
    )]);
    let ctx = contracted_identity_ctx(&project, &env, true, http.clone());
    let wanted = crate::seed::said(&crate::seed::wanted_telling());
    let mut baseline = crate::baseline::Baseline::new();
    baseline.record(
        CONTRACTED_REQUESTS,
        crate::seed::TELLING,
        &wanted,
        "2026-08-28T00:00:00Z",
    );
    super::super::save_baseline(&ctx, &baseline);

    let (requests, recorded) = super::super::managed_telling(
        &ctx,
        &beside_contracted_requests(&project, &first_party("intake")),
    )
    .await;

    let told = match requests {
        Some(requests) => requests.telling().await.ok(),
        None => None,
    };
    assert_eq!(told.map(|telling| telling.enabled), Some(true));
    assert_eq!(recorded.map(|record| record.value), Some(wanted));
    assert_eq!(
        http.requests()
            .into_iter()
            .map(|one| one.url)
            .collect::<Vec<_>>(),
        vec![format!("{CONTRACTED_REQUESTS_AT}telling")]
    );
}

/// A stack nothing on which fills `request.intake` has no telling to ask about, and no
/// record to hold it to, whatever an earlier run recorded.
#[tokio::test]
async fn without_a_request_service_there_is_no_telling_to_ask_about() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("telling-unfilled");
    let env = config_scratch("telling-unfilled");
    let http = Fake::always(Answer::reply(200, "{}"));
    let ctx = contracted_identity_ctx(&project, &env, true, http.clone());
    let mut baseline = crate::baseline::Baseline::new();
    baseline.record(
        "seerr",
        crate::seed::TELLING,
        &crate::seed::said(&crate::seed::wanted_telling()),
        "2026-08-28T00:00:00Z",
    );
    super::super::save_baseline(&ctx, &baseline);

    let (requests, recorded) =
        super::super::managed_telling(&ctx, &fillers_at(vec![media_server_svc()], &project)).await;

    assert!(requests.is_none());
    assert!(recorded.is_none());
    assert!(http.requests().is_empty());
}
