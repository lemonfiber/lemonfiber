//! A plugin's gate standing in for the stack's over `request.gate`: the routes it is
//! handed over the contract, and what every step reaching through it does where it cannot
//! be asked.

use crate::app::gating::SERVICE;
use crate::test_support::{a_contracted_gate, unaskable_gates, GATE_TRUSTED};

use super::gate::routes;
use super::*;

/// What the gate is asked for the routes it admits.
const ADMITTED: &str = "/lemonfiber/request.gate/v1/admitted";

/// What the gate is told every route it should admit through.
const ADMIT: &str = "/lemonfiber/request.gate/v1/admit";

/// [`a_contracted_gate`] beside the request service and both curators, trusting `trusted`
/// as first-party.
fn contracted_gate(
    project: &std::path::Path,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    fillers_trusting(
        vec![
            requests_svc(),
            curator("sonarr", 8989, "tv"),
            curator("radarr", 7878, "movies"),
        ],
        &[a_contracted_gate()],
        project,
        trusted,
    )
}

/// The path and credential of every route each `admit` the gate was told carried.
fn admitted_routes(http: &Fake) -> Vec<(String, String)> {
    http.requests()
        .iter()
        .filter(|one| one.url.ends_with(ADMIT))
        .filter_map(|one| serde_json::from_str::<Admitting>(one.body.as_deref()?).ok())
        .flat_map(|admitting| admitting.routes)
        .map(|route| (route.path, route.credential))
        .collect()
}

/// A gate answering it admits nothing and taking every route it is handed.
fn admitting_nothing() -> Arc<Fake> {
    Fake::by_path(vec![
        (ADMITTED, Answer::reply(200, "[]")),
        (ADMIT, Answer::reply(204, "")),
    ])
}

/// The stack the contracted gate stands in for: it still names its own gate.
fn standing_in() -> Vec<lemonfiber_manifest::Service> {
    vec![
        requests_svc(),
        curator("sonarr", 8989, "tv"),
        curator("radarr", 7878, "movies"),
        request_gate(),
    ]
}

/// The states `wirings` settled in, in order.
fn states(wirings: &[Wiring]) -> Vec<&State> {
    wirings.iter().map(|one| &one.state).collect()
}

/// Whether anything but a read was asked of anyone.
fn wrote(http: &Fake) -> bool {
    http.requests()
        .iter()
        .any(|asked| asked.method != Method::Get)
}

#[tokio::test]
async fn a_first_party_gate_speaking_the_contract_is_handed_its_routes_over_it() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("gate-contracted");
    let http = admitting_nothing();
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let fillers = contracted_gate(&project, &GATE_TRUSTED);

    let wirings =
        super::super::gate::seed_gate_routes(&ctx, &standing_in(), &fillers, None, Some(&project))
            .await;

    assert_eq!(states(&wirings), vec![&State::Wired]);
    assert!(http.requests().iter().all(|one| one
        .url
        .starts_with("http://127.0.0.1:8080/lemonfiber/request.gate/v1/")));
    assert_eq!(
        admitted_routes(&http),
        vec![
            ("sonarr".to_owned(), "the-key".to_owned()),
            ("radarr".to_owned(), "the-key".to_owned()),
        ]
    );
    assert_eq!(routes(&project), None);
}

/// A curator a third-party plugin brought is not routed through a first-party gate: its
/// key may not be handed to another plugin's service.
#[tokio::test]
async fn a_third_party_curators_key_is_never_handed_to_the_gate() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("gate-stranger-curator");
    let http = admitting_nothing();
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let mut stranger = crate::test_support::a_placed(
        "kept",
        &["library.curate"],
        Some(curator_api(Some("/config/config.xml"))),
        Some(8990),
    );
    stranger.media_types = vec!["movies".to_owned()];
    let fillers = fillers_trusting(
        vec![requests_svc(), curator("sonarr", 8989, "tv")],
        &[
            a_contracted_gate(),
            crate::test_support::an_installed("kept", vec![stranger]),
        ],
        &project,
        &GATE_TRUSTED,
    );

    let _ =
        super::super::gate::seed_gate_routes(&ctx, &standing_in(), &fillers, None, Some(&project))
            .await;

    assert_eq!(
        admitted_routes(&http),
        vec![("sonarr".to_owned(), "the-key".to_owned())]
    );
}

/// What one `admit` carried.
#[derive(serde::Deserialize)]
struct Admitting {
    /// Every route, credentials and all.
    routes: Vec<crate::ports::service::Gated>,
}

#[tokio::test]
async fn a_gate_that_cannot_be_asked_over_its_contract_is_handed_nothing() {
    for (tag, trusted, keyed) in unaskable_gates() {
        let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("gate-unasked-{tag}"));
        let http = Fake::always(Answer::reply(200, "[]"));
        let ctx = contracted_ctx(&project, SERVICE, keyed, http.clone());
        let fillers = contracted_gate(&project, trusted);

        let wirings = super::super::gate::seed_gate_routes(
            &ctx,
            &standing_in(),
            &fillers,
            None,
            Some(&project),
        )
        .await;

        assert!(
            wirings
                .iter()
                .all(|one| matches!(one.state, State::Skipped { .. })),
            "{tag}: {wirings:?}"
        );
        assert!(http.requests().is_empty(), "{tag}: {:?}", http.requests());
        assert_eq!(routes(&project), None, "{tag}");
    }
}

/// A gate that refuses to say what it admits is reported in its own words, and handed
/// no route.
#[tokio::test]
async fn a_gate_that_cannot_say_what_it_admits_is_reported_and_handed_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("gate-contracted-unread");
    let http = Fake::by_path(vec![(ADMITTED, Answer::reply(500, ""))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let fillers = contracted_gate(&project, &GATE_TRUSTED);

    let wirings =
        super::super::gate::seed_gate_routes(&ctx, &standing_in(), &fillers, None, Some(&project))
            .await;

    assert_eq!(
        states(&wirings),
        vec![&State::Failed {
            detail: "500 is not an answer the contract declares".to_owned(),
        }]
    );
    assert!(
        !http.requests().iter().any(|one| one.url.ends_with(ADMIT)),
        "{:?}",
        http.requests()
    );
    assert_eq!(routes(&project), None);
}

/// A request service reaching each curator through a gate that cannot be asked over its
/// contract is handed none of them: each fails, saying why, and nothing is written.
#[tokio::test]
async fn without_a_gate_to_ask_no_curator_is_handed_to_the_request_service() {
    let unasked = State::Failed {
        detail: super::super::tokens::UNASKED.to_owned(),
    };
    for (tag, trusted, keyed) in unaskable_gates() {
        let project =
            lemonfiber_fixtures::scratch::Scratch::new(&format!("gate-unasked-fulfilment-{tag}"));
        let http = Fake::by_path(vec![
            (
                "/qualityprofile",
                Answer::reply(200, r#"[{"id":4,"name":"HD-1080p"}]"#),
            ),
            (
                "/rootfolder",
                Answer::reply(200, r#"[{"id":1,"path":"/data/media/tv"}]"#),
            ),
            ("/settings/", Answer::reply(200, "[]")),
        ]);
        let ctx = contracted_ctx(&project, SERVICE, keyed, http.clone());
        let fillers = contracted_gate(&project, trusted);

        let wirings =
            super::super::seed_fulfilment_targets(&ctx, &standing_in(), &fillers, Some(&project))
                .await;

        assert_eq!(states(&wirings), vec![&unasked, &unasked], "{tag}");
        assert!(!wrote(&http), "{tag}: {:?}", http.requests());
    }
}

/// The request service's connection to the media server, held at a gate that cannot be
/// asked over its contract, fails saying why, and is handed no token.
#[tokio::test]
async fn without_a_gate_to_ask_the_media_server_link_is_handed_no_token() {
    for (tag, trusted, keyed) in unaskable_gates() {
        let project =
            lemonfiber_fixtures::scratch::Scratch::new(&format!("gate-unasked-link-{tag}"));
        let http = Fake::by_path(vec![(
            "/settings/jellyfin",
            Answer::reply(
                200,
                r#"{"ip":"jellyfin","port":8096,"useSsl":false,"urlBase":"","apiKey":"own"}"#,
            ),
        )]);
        let ctx = contracted_ctx(&project, SERVICE, keyed, http.clone());
        let trusting: Vec<_> = trusted
            .iter()
            .chain(
                crate::plugin::first_party::EMBEDDED
                    .iter()
                    .filter(|one| one.plugin == crate::test_support::media::PLUGIN),
            )
            .copied()
            .collect();
        let fillers = fillers_trusting(
            vec![media_server_svc(), requests_svc()],
            &std::iter::once(a_contracted_gate())
                .chain(crate::test_support::media::moved())
                .collect::<Vec<_>>(),
            &project,
            &trusting,
        );

        assert_eq!(
            linked(&ctx, &fillers, &project).await,
            Some(State::Failed {
                detail: super::super::tokens::UNASKED.to_owned(),
            }),
            "{tag}"
        );
        assert!(!wrote(&http), "{tag}: {:?}", http.requests());
    }
}

/// What holding the request service of `fillers` to its media server link through the gate
/// under `project` comes to, where both are there.
async fn linked(
    ctx: &Ctx,
    fillers: &crate::wiring::Fillers,
    project: &std::path::Path,
) -> Option<State> {
    let asker = crate::app::targets::request_service(fillers)?;
    let server = crate::app::targets::MediaServer::of(fillers)?;
    let requests = crate::app::targets::requests_as_owner(ctx, asker).await?;
    Some(
        super::super::linking::seed_media_server_link(
            ctx,
            fillers,
            requests.as_ref(),
            asker,
            &server,
            project,
        )
        .await
        .state,
    )
}
