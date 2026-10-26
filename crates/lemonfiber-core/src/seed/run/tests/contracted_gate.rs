//! A plugin's gate standing in for the stack's over `request.gate`: the routes it is
//! handed over the contract, and what every step reaching through it does where it cannot
//! be asked.

use crate::app::gating::SERVICE;
use crate::test_support::{a_contracted_gate, unaskable_gates, GATE_TRUSTED};

use super::gate::routes;
use super::*;
use crate::test_support::curators::{self, FILM, TELEVISION};

/// What the gate is asked for the routes it admits.
const ADMITTED: &str = "/lemonfiber/request.gate/v1/admitted";

/// What the gate is told every route it should admit through.
const ADMIT: &str = "/lemonfiber/request.gate/v1/admit";

/// [`a_contracted_gate`] beside the request service moved into its plugin and both
/// curators, trusting `trusted` as first-party beside every plugin this build embeds.
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
        &with_the_request_service(vec![a_contracted_gate()]),
        project,
        &trusting(trusted),
    )
}

/// `installed`, with the request service moved into its plugin.
fn with_the_request_service(
    installed: Vec<crate::plugin::Installed>,
) -> Vec<crate::plugin::Installed> {
    installed
        .into_iter()
        .chain(crate::test_support::requests::REQUESTS.installed())
        .collect()
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

/// Whether anybody was asked to change anything: the gate to admit a route, or the
/// request service to hold a curator or a link.
fn wrote(http: &Fake) -> bool {
    use crate::test_support::requests::intake;
    let writes = [
        ADMIT,
        intake("add_fulfilment_target"),
        intake("move_fulfilment_target"),
        intake("link_media_server"),
    ];
    http.requests()
        .iter()
        .any(|asked| writes.iter().any(|write| asked.url.ends_with(write)))
}

/// `trusted` as first-party, beside every plugin this build embeds.
fn trusting(
    trusted: &[crate::plugin::first_party::FirstParty],
) -> Vec<crate::plugin::first_party::FirstParty> {
    trusted
        .iter()
        .chain(crate::plugin::first_party::EMBEDDED)
        .copied()
        .collect()
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

/// A third-party plugin's curator filing film, beside the request service moved into its
/// plugin holding `granted`, the first-party gate and the television curator.
fn beside_a_stranger(
    project: &std::path::Path,
    granted: &[crate::plugin::Grant],
) -> crate::wiring::Fillers {
    let mut stranger = crate::test_support::a_placed(
        "kept",
        &["library.curate"],
        Some(curator_api(Some("/config/config.xml"))),
        Some(8990),
    );
    stranger.media_types = vec!["movies".to_owned()];
    let requests = REQUESTS.installed().map(|mut one| {
        granted.clone_into(&mut one.granted);
        one
    });
    fillers_trusting(
        vec![requests_svc(), curator("sonarr", 8989, "tv")],
        &[
            Some(a_contracted_gate()),
            Some(crate::test_support::an_installed("kept", vec![stranger])),
            requests,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>(),
        project,
        &trusting(&GATE_TRUSTED),
    )
}

/// A curator a third-party plugin brought is not routed through a first-party gate: its
/// key may not be handed to another plugin's service. Where the operator granted it to the
/// request service, so the request service fulfils through it, the gate's pass says it
/// holds no route to it and why.
#[tokio::test]
async fn a_third_party_curators_key_is_never_handed_to_the_gate() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("gate-stranger-curator");
    let http = admitting_nothing();
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let ungranted = beside_a_stranger(&project, &[]);
    let credential = ungranted
        .service("kept")
        .and_then(|kept| ungranted.credential(kept))
        .unwrap_or_default();
    let granted = beside_a_stranger(
        &project,
        &[crate::plugin::Grant {
            credential,
            of: "kept".to_owned(),
            serves: "request.intake".to_owned(),
            at: String::new(),
        }],
    );

    let alone = super::super::gate::seed_gate_routes(
        &ctx,
        &standing_in(),
        &ungranted,
        None,
        Some(&project),
    )
    .await;
    let said =
        super::super::gate::seed_gate_routes(&ctx, &standing_in(), &granted, None, Some(&project))
            .await;

    assert_eq!(
        admitted_routes(&http),
        vec![
            ("sonarr".to_owned(), "the-key".to_owned()),
            ("sonarr".to_owned(), "the-key".to_owned())
        ]
    );
    assert_eq!(states(&alone), vec![&State::Wired]);
    assert!(
        matches!(said.as_slice(), [_, unrouted]
            if unrouted.connection == "kept the stand-in routed through the request gate"
                && matches!(&unrouted.state, State::Unmatched { reason }
                    if reason.starts_with("kept the stand-in is a third-party plugin's service"))),
        "{said:?}"
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
        let folder = |path: &str| crate::ports::service::RegisteredFolder {
            id: "1".to_owned(),
            path: path.to_owned(),
        };
        let profiles = curators::answered(&[crate::ports::service::QualityProfile {
            id: 4,
            name: "HD-1080p".to_owned(),
        }]);
        let http = Fake::by_path(vec![
            (
                curators::curating(TELEVISION, "quality_profiles"),
                profiles.clone(),
            ),
            (curators::curating(FILM, "quality_profiles"), profiles),
            (
                curators::curating(TELEVISION, "root_folders"),
                curators::answered(&[folder("/data/media/tv")]),
            ),
            (
                curators::curating(FILM, "root_folders"),
                curators::answered(&[folder("/data/media/movies")]),
            ),
            (
                crate::test_support::requests::intake("fulfilment_targets"),
                curators::answered(&Vec::<crate::ports::service::RegisteredTarget>::new()),
            ),
        ]);
        let mut ctx = contracted_ctx(&project, SERVICE, keyed, http.clone());
        ctx.settings.env_file = Some(project.join(".env"));
        let ctx = crate::test_support::adapted::asking(ctx, &[TELEVISION, FILM, REQUESTS]);
        for curator in [TELEVISION, FILM] {
            curator.minted(&ctx, MINTED_CURATOR_KEY);
        }
        let installed: Vec<_> = [
            Some(a_contracted_gate()),
            TELEVISION.installed(),
            FILM.installed(),
            REQUESTS.installed(),
        ]
        .into_iter()
        .flatten()
        .collect();
        let fillers = fillers_trusting(
            vec![request_gate()],
            &installed,
            &project,
            &trusting(trusted),
        );

        let wirings = super::super::seed_fulfilment_targets(
            &ctx,
            &[request_gate()],
            &fillers,
            Some(&project),
        )
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
            crate::test_support::requests::intake("media_server_link"),
            crate::test_support::requests::answered(&crate::ports::service::MediaServerLink {
                at: crate::ports::service::Endpoint {
                    host: "jellyfin".to_owned(),
                    port: 8096,
                    base: String::new(),
                },
                key: "own".to_owned(),
            }),
        )]);
        let ctx = crate::test_support::adapted::asking(
            contracted_ctx(&project, SERVICE, keyed, http.clone()),
            &[REQUESTS],
        );
        let installed: Vec<_> = [
            Some(a_contracted_gate()),
            crate::test_support::media::moved(),
            REQUESTS.installed(),
        ]
        .into_iter()
        .flatten()
        .collect();
        let fillers = fillers_trusting(
            vec![media_server_svc()],
            &installed,
            &project,
            &trusting(trusted),
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
