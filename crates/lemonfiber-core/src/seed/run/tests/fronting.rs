//! The household's proxy, applied a route for every household service of every plugin.

use super::super::fronting::{refront, seed_routes, served};
use super::*;
use crate::ports::service::{Route, Routing, Tls};

/// Where a proxy is asked what it serves.
const ROUTING: &str = "/lemonfiber/proxy.front/v1/routing";

/// Where a proxy is handed a routing table.
const APPLY: &str = "/lemonfiber/proxy.front/v1/apply";

/// A plugin `routed` whose service `proxy`, which the household reaches, speaks
/// `proxy.front` where `speaking`: the proxy, which is given no route to itself.
fn the_proxy(speaking: bool) -> crate::plugin::Installed {
    let mut installed = crate::test_support::contracted("routed", "proxy", "proxy.front");
    for placed in &mut installed.services {
        placed.reached = Some(crate::plugin::Reached::Household {
            port: 80,
            hostname: "front".to_owned(),
            group: None,
        });
        if !speaking {
            placed.speaks.clear();
        }
    }
    installed
}

/// A plugin whose one service the household reaches at `comics`.
fn the_comics() -> crate::plugin::Installed {
    let mut placed = crate::test_support::a_placed("komga", &[], Some(25600));
    placed.reached = Some(crate::plugin::Reached::Household {
        port: 25600,
        hostname: "comics".to_owned(),
        group: None,
    });
    crate::test_support::an_installed("comics", vec![placed])
}

/// The route the comics are served at, with no domain recorded.
fn comics_route() -> Route {
    Route {
        name: "comics.home.local".to_owned(),
        host: "komga".to_owned(),
        port: 25600,
    }
}

/// A table holding `routes`, its TLS obtained by the proxy.
fn table(routes: Vec<Route>) -> String {
    serde_json::to_string(&Routing {
        tls: Tls::Automatic,
        routes,
    })
    .unwrap_or_default()
}

/// A proxy answering what it serves with each of `served` in turn, and every table it is
/// handed with `applied`.
fn proxying(served: Vec<Answer>, applied: Answer) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![(ROUTING, served), (APPLY, vec![applied])])
}

/// Every table a proxy was handed.
fn applied(http: &Fake) -> Vec<Routing> {
    #[derive(serde::Deserialize)]
    struct Told {
        table: Routing,
    }
    http.requests()
        .into_iter()
        .filter(|one| one.url.ends_with(APPLY))
        .filter_map(|one| serde_json::from_str::<Told>(one.body.as_deref()?).ok())
        .map(|told| told.table)
        .collect()
}

/// [`the_proxy`] beside [`the_comics`].
fn installed(speaking: bool) -> Vec<crate::plugin::Installed> {
    vec![the_proxy(speaking), the_comics()]
}

/// Who fills each ask with `installed` beside no stack service, `routed` trusted.
fn filled(
    project: &std::path::Path,
    installed: &[crate::plugin::Installed],
) -> crate::wiring::Fillers {
    fillers_trusting(
        Vec::new(),
        installed,
        project,
        &beside_the_bundle(&first_party("routed")),
    )
}

/// What the step settled in, where it ran.
async fn seeded(ctx: &Ctx, project: &std::path::Path, speaking: bool) -> Option<State> {
    let installed = installed(speaking);
    seed_routes(
        ctx,
        &[],
        &installed,
        &filled(project, &installed),
        &crate::baseline::Baseline::new(),
    )
    .await
    .map(|wiring| wiring.state)
}

#[tokio::test]
async fn the_proxy_is_applied_a_route_for_every_household_service_and_then_holds_it() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("front-applied");
    let http = proxying(
        vec![
            Answer::reply(200, table(Vec::new())),
            Answer::reply(200, table(vec![comics_route()])),
        ],
        Answer::reply(204, ""),
    );
    let ctx = contracted_ctx(&project, "proxy", true, http.clone());

    assert_eq!(seeded(&ctx, &project, true).await, Some(State::Wired));
    assert_eq!(
        applied(&http),
        vec![Routing {
            tls: Tls::Automatic,
            routes: vec![comics_route()],
        }]
    );
    assert_eq!(
        seeded(&ctx, &project, true).await,
        Some(State::AlreadyWired)
    );
    assert_eq!(applied(&http).len(), 1, "nothing was handed over again");
}

#[tokio::test]
async fn a_rehearsal_says_which_names_it_would_serve_and_hands_over_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("front-rehearsed");
    let http = proxying(
        vec![Answer::reply(200, table(Vec::new()))],
        Answer::reply(204, ""),
    );
    let mut ctx = contracted_ctx(&project, "proxy", true, http.clone());
    ctx.dry_run = true;

    let state = seeded(&ctx, &project, true).await;

    assert!(
        matches!(&state, Some(State::WouldWire { yours: Some(yours), ours: Some(ours) })
            if yours == "no names" && ours.starts_with("comics.home.local")),
        "{state:?}"
    );
    assert!(applied(&http).is_empty());
}

#[tokio::test]
async fn a_proxy_that_does_not_answer_or_refuses_the_table_is_said_to() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("front-unanswered");
    let silent = proxying(vec![Answer::reply(500, "")], Answer::reply(204, ""));
    let ctx = contracted_ctx(&project, "proxy", true, silent.clone());
    assert!(matches!(
        seeded(&ctx, &project, true).await,
        Some(State::Failed { .. })
    ));
    assert!(applied(&silent).is_empty());

    let refusing = proxying(
        vec![Answer::reply(200, table(Vec::new()))],
        Answer::reply(500, ""),
    );
    let project = lemonfiber_fixtures::scratch::Scratch::new("front-refused");
    let ctx = contracted_ctx(&project, "proxy", true, refusing);
    let state = seeded(&ctx, &project, true).await;
    assert!(matches!(&state, Some(State::Failed { .. })), "{state:?}");
}

#[tokio::test]
async fn nothing_is_said_where_nothing_speaks_proxy_front() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("front-unspoken");
    let http = proxying(
        vec![Answer::reply(200, table(Vec::new()))],
        Answer::reply(204, ""),
    );
    let ctx = contracted_ctx(&project, "proxy", true, http.clone());

    assert_eq!(seeded(&ctx, &project, false).await, None);
    let comics = vec![the_comics()];
    assert_eq!(
        seed_routes(
            &ctx,
            &[],
            &comics,
            &filled(&project, &comics),
            &crate::baseline::Baseline::new()
        )
        .await,
        None
    );
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn the_proxy_is_applied_again_over_what_is_installed_and_read_back() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("front-refronted");
    let http = proxying(
        vec![Answer::reply(200, table(vec![comics_route()]))],
        Answer::reply(204, ""),
    );
    let ctx = contracted_ctx(&project, "proxy", true, http.clone());
    let stack =
        project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref()).unwrap_or_default();
    let key = crate::plugin::key_file(&stack, "proxy");
    let _ = std::fs::create_dir_all(key.parent().unwrap_or(&stack));
    let _ = std::fs::write(&key, crate::test_support::CONTRACTED_KEY);
    assert!(crate::config::store::set(
        ctx.settings.env_file.as_deref().unwrap_or(project.as_ref()),
        crate::wiring::FILLS_KEY,
        "proxy.front=proxy",
    )
    .is_ok());
    let kept = crate::app::targets::beside_env(&ctx, crate::config::paths::PLUGINS);
    let mut register = crate::plugin::Register::empty();
    for one in installed(true) {
        assert!(register.record(one).is_ok());
    }
    assert!(crate::app::record::keep(kept.as_deref(), &register).is_ok());

    assert_eq!(refront(&ctx).await, Some(State::Wired));
    assert!(applied(&http)
        .iter()
        .all(|table| table.routes.contains(&comics_route())
            && table.routes.iter().all(|route| route.host != "proxy")));
    assert_eq!(served(&ctx).await, vec![comics_route()]);
}

#[tokio::test]
async fn nothing_is_applied_or_read_where_the_stack_or_its_proxy_cannot_be() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("front-unread");
    let http = proxying(
        vec![Answer::reply(200, table(Vec::new()))],
        Answer::reply(204, ""),
    );
    let ctx = contracted_ctx(&project, "proxy", true, http.clone());

    assert_eq!(refront(&ctx).await, None);
    assert!(served(&ctx).await.is_empty());

    let mut stackless = ctx;
    stackless.stack = crate::test_support::nowhere();
    assert_eq!(refront(&stackless).await, None);
    assert!(served(&stackless).await.is_empty());
    assert!(http.requests().is_empty());
}

/// The comics, claimed against an account outside the house through an adapter handed a
/// code that lapses.
fn the_coded_comics() -> crate::plugin::Installed {
    let mut installed = crate::test_support::contracted("comics", "komga-adapter", "owner.claim");
    for adapter in &mut installed.services {
        adapter.provides.clear();
        adapter.fronts = Some("komga".to_owned());
        adapter
            .upstream
            .insert("claim_code".to_owned(), "claim-code".to_owned());
    }
    installed.services.extend(the_comics().services);
    installed.declared.secrets = vec![crate::plugin::Secret {
        id: "claim-code".to_owned(),
        of: "komga".to_owned(),
        why: "The code the account shows".to_owned(),
        minted: false,
        serves: None,
        was: None,
        needs: lemonfiber_manifest::container::choices::Needs::new(),
        needed_to_start: false,
        lapses_s: Some(240),
    }];
    installed
}

/// A baseline recording a claim of the comics as `claimed`, where it records one.
fn claimed(claimed: Option<&str>) -> crate::baseline::Baseline {
    let mut baseline = crate::baseline::Baseline::new();
    if let Some(claimed) = claimed {
        baseline.record(
            "komga",
            super::super::owning::FIELD,
            claimed,
            "2026-10-31T00:00:00Z",
        );
    }
    baseline
}

#[tokio::test]
async fn a_part_not_yet_the_households_has_no_route_until_its_claim_is_recorded_and_drift_drops_it()
{
    for (name, recorded, routed) in [
        ("unclaimed", None, false),
        ("claimed", Some("ours"), true),
        ("reopened", Some("open"), false),
    ] {
        let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("front-coded-{name}"));
        let http = proxying(
            vec![Answer::reply(200, table(Vec::new()))],
            Answer::reply(204, ""),
        );
        let ctx = contracted_ctx(&project, "proxy", true, http.clone());
        let installed = vec![the_proxy(true), the_coded_comics()];
        let trusted: Vec<_> = first_party("routed")
            .into_iter()
            .chain(first_party("comics"))
            .collect();
        let fillers = fillers_trusting(
            Vec::new(),
            &installed,
            &project,
            &beside_the_bundle(&trusted),
        );

        let state = seed_routes(&ctx, &[], &installed, &fillers, &claimed(recorded)).await;

        let routes: Vec<Route> = applied(&http)
            .into_iter()
            .flat_map(|table| table.routes)
            .collect();
        let expected = if routed {
            vec![comics_route()]
        } else {
            Vec::new()
        };
        assert_eq!(routes, expected, "{name}: {state:?}");
    }
}
