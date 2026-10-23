//! The dashboard, applied the panel list for every service on the machine.

use super::super::showing::{reshow, seed_dashboard};
use super::*;

/// The shipped services file the dashboard starts with.
const SHIPPED: &str = "- Watch:\n    - Jellyfin:\n        href: http://x:8096\n";

/// A stack directory holding the dashboard's shipped files, and a context whose settings
/// sit beside it, rehearsing where `dry_run`.
fn dashboard(name: &str, dry_run: bool) -> (lemonfiber_fixtures::scratch::Scratch, Ctx) {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new(name);
    for file in ["services.yaml", "widgets.yaml"] {
        let at = scratch.path().join("config/homepage").join(file);
        let _ = at.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(at, SHIPPED);
    }
    let env = scratch.path().join(".env");
    let _ = std::fs::write(&env, "");
    let mut ctx = a_context()
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build();
    ctx.dry_run = dry_run;
    (scratch, ctx)
}

/// The stack this build carries.
fn carried() -> Vec<lemonfiber_manifest::Service> {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

/// What the dashboard's services file holds.
fn services_file(scratch: &lemonfiber_fixtures::scratch::Scratch) -> String {
    std::fs::read_to_string(scratch.path().join("config/homepage/services.yaml"))
        .unwrap_or_default()
}

/// The state the step settled in, where it ran.
async fn seeded(ctx: &Ctx, project: &std::path::Path) -> Option<State> {
    let services = carried();
    let fillers = fillers_at(services.clone(), project);
    seed_dashboard(ctx, &services, &[], &fillers, Some(project))
        .await
        .map(|wiring| wiring.state)
}

#[tokio::test]
async fn the_dashboard_is_applied_the_panel_list_once_and_then_holds_it() {
    let (scratch, ctx) = dashboard("dashboard-applied", false);

    assert_eq!(seeded(&ctx, scratch.path()).await, Some(State::Wired));
    let written = services_file(&scratch);
    assert_ne!(written, SHIPPED);
    assert_eq!(
        seeded(&ctx, scratch.path()).await,
        Some(State::AlreadyWired)
    );
    assert_eq!(services_file(&scratch), written);
}

#[tokio::test]
async fn the_dashboard_is_applied_again_over_the_stack_and_what_is_installed() {
    let (scratch, mut ctx) = dashboard("dashboard-reshown", false);

    assert_eq!(reshow(&ctx).await, Some(State::Wired));

    ctx.settings.unmanaged = vec![("homepage".to_owned(), "mine".to_owned())];
    assert_eq!(reshow(&ctx).await, None);
    ctx.settings.unmanaged.clear();

    let _ = std::fs::write(
        scratch.path().join(crate::config::paths::PLUGINS),
        "not a register",
    );
    assert_eq!(reshow(&ctx).await, None);

    let mut stackless = ctx;
    stackless.stack = crate::test_support::nowhere();
    assert_eq!(reshow(&stackless).await, None);
}

#[tokio::test]
async fn a_rehearsal_says_what_it_would_show_and_writes_nothing() {
    let (scratch, ctx) = dashboard("dashboard-rehearsed", true);

    let state = seeded(&ctx, scratch.path()).await;

    assert!(
        matches!(&state, Some(State::WouldWire { yours: Some(yours), ours: Some(ours) })
            if yours.starts_with("0 entries") && ours.contains("entries in")),
        "{state:?}"
    );
    assert_eq!(services_file(&scratch), SHIPPED);
}

#[tokio::test]
async fn a_dashboard_the_operator_declared_unmanaged_is_skipped_and_left_alone() {
    let (scratch, mut ctx) = dashboard("dashboard-unmanaged", false);
    ctx.settings.unmanaged = vec![("config/homepage".to_owned(), "mine".to_owned())];

    let state = seeded(&ctx, scratch.path()).await;

    assert!(matches!(state, Some(State::Skipped { .. })), "{state:?}");
    assert_eq!(services_file(&scratch), SHIPPED);
}

#[tokio::test]
async fn a_search_address_that_is_not_one_fails_naming_the_setting() {
    let (scratch, ctx) = dashboard("dashboard-searching", false);
    let settings = scratch.path().join(crate::config::paths::SETTINGS);
    let _ = crate::settings::store::recorded(
        &settings,
        crate::settings::own::DASHBOARD_SEARCH,
        Some("duckduckgo.com"),
    );

    let state = seeded(&ctx, scratch.path()).await;

    assert!(
        matches!(&state, Some(State::Failed { detail })
            if detail.starts_with(crate::settings::own::DASHBOARD_SEARCH)),
        "{state:?}"
    );
    assert_eq!(services_file(&scratch), SHIPPED);
}

#[tokio::test]
async fn a_stack_running_no_dashboard_is_not_asked() {
    let (scratch, ctx) = dashboard("dashboard-absent", false);
    let services: Vec<lemonfiber_manifest::Service> = carried()
        .into_iter()
        .filter(|one| {
            !one.provides
                .iter()
                .any(|provided| provided == "dashboard.show")
        })
        .collect();
    let fillers = fillers_at(services.clone(), scratch.path());

    assert!(
        seed_dashboard(&ctx, &services, &[], &fillers, Some(scratch.path()))
            .await
            .is_none()
    );
    assert!(seed_dashboard(&ctx, &services, &[], &fillers, None)
        .await
        .is_none());
}

/// The plugin a contract-speaking dashboard is installed as.
const SHOWER: &str = "shower";

/// What the dashboard is asked for the panel list it shows.
const PANELS: &str = "/lemonfiber/dashboard.show/v1/panels";

/// What the dashboard is told the panel list to show.
const APPLY: &str = "/lemonfiber/dashboard.show/v1/apply";

/// [`SHOWER`] as first-party.
const SHOWER_TRUSTED: [crate::plugin::first_party::FirstParty; 1] =
    crate::test_support::first_party(SHOWER);

/// The stack this build carries with no dashboard of its own.
fn undashboarded() -> Vec<lemonfiber_manifest::Service> {
    carried()
        .into_iter()
        .filter(|one| {
            !one.provides
                .iter()
                .any(|provided| provided == "dashboard.show")
        })
        .collect()
}

/// A plugin's dashboard beside the stack, speaking `dashboard.show` where `speaking`,
/// trusting `trusted` as first-party.
fn beside(
    project: &std::path::Path,
    speaking: bool,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    let mut plugin = crate::test_support::contracted(SHOWER, SHOWER, "dashboard.show");
    if !speaking {
        for placed in &mut plugin.services {
            placed.speaks.clear();
        }
    }
    fillers_trusting(undashboarded(), &[plugin], project, trusted)
}

#[tokio::test]
async fn a_first_party_dashboard_speaking_the_contract_is_applied_the_panel_list_over_it() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("dashboard-contracted");
    let http = Fake::by_path(vec![
        (PANELS, Answer::reply(200, r#"{"groups":[],"widgets":[]}"#)),
        (APPLY, Answer::reply(204, "")),
    ]);
    let ctx = contracted_ctx(&project, SHOWER, true, http.clone());
    let fillers = beside(&project, true, &SHOWER_TRUSTED);

    let state = seed_dashboard(&ctx, &undashboarded(), &[], &fillers, Some(&project))
        .await
        .map(|wiring| wiring.state);

    assert_eq!(state, Some(State::Wired));
    assert!(http.requests().iter().any(|one| one.url.ends_with(APPLY)));
}

#[tokio::test]
async fn a_dashboard_that_cannot_be_asked_is_reported_or_passed_over() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("dashboard-unasked");
    let refusing = Fake::by_path(vec![(PANELS, Answer::reply(503, ""))]);
    let ctx = contracted_ctx(&project, SHOWER, true, refusing.clone());

    let unreached = seed_dashboard(
        &ctx,
        &undashboarded(),
        &[],
        &beside(&project, true, &SHOWER_TRUSTED),
        Some(&project),
    )
    .await
    .map(|wiring| wiring.state);
    assert!(
        matches!(unreached, Some(ref state) if !state.is_settled() || matches!(state, State::Failed { .. })),
        "{unreached:?}"
    );

    for (named, speaking, keyed) in [("unkeyed", true, false), ("speaking nothing", false, true)] {
        let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("dashboard-{named}"));
        let http = Fake::by_path(Vec::new());
        let ctx = contracted_ctx(&project, SHOWER, keyed, http.clone());
        let fillers = beside(&project, speaking, &SHOWER_TRUSTED);
        assert!(
            seed_dashboard(&ctx, &undashboarded(), &[], &fillers, Some(&project))
                .await
                .is_none(),
            "{named}"
        );
        assert!(http.requests().is_empty(), "{named}");
    }
}

#[tokio::test]
async fn a_dashboard_that_will_not_take_the_panel_list_fails_saying_so() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("dashboard-refusing");
    let http = Fake::by_path(vec![
        (PANELS, Answer::reply(200, r#"{"groups":[],"widgets":[]}"#)),
        (APPLY, Answer::reply(500, "")),
    ]);
    let ctx = contracted_ctx(&project, SHOWER, true, http);
    let fillers = beside(&project, true, &SHOWER_TRUSTED);

    let state = seed_dashboard(&ctx, &undashboarded(), &[], &fillers, Some(&project))
        .await
        .map(|wiring| wiring.state);

    assert!(
        matches!(&state, Some(State::Failed { detail }) if detail.contains("could not be applied")),
        "{state:?}"
    );
}
