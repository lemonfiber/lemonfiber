//! The dashboard, applied the panel list over `dashboard.show` for every service on the
//! machine.

use super::super::showing::{reshow, seed_dashboard};
use super::*;

/// The stack this build carries.
fn carried() -> Vec<lemonfiber_manifest::Service> {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

/// What the dashboard answers it shows where it shows nothing.
const NOTHING: &str = r#"{"groups":[],"widgets":[]}"#;

/// The panel list a seed pass applies over the stack this build carries with no
/// dashboard of its own, beside nothing installed.
fn wanted(ctx: &Ctx, fillers: &crate::wiring::Fillers) -> String {
    let search = crate::settings::dashboard_search(ctx).unwrap_or_default();
    serde_json::to_string(&crate::app::showing::panels::panels(
        &undashboarded(),
        &[],
        fillers,
        &crate::door::run::place(ctx, None),
        &search,
    ))
    .unwrap_or_default()
}

/// A dashboard answering what it shows with each of `shown` in turn, and every list it
/// is told to show with `applied`.
fn showing_in_turn(shown: Vec<Answer>, applied: Answer) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![(PANELS, shown), (APPLY, vec![applied])])
}

/// What the step settled in over [`beside`], where it ran.
async fn seeded(ctx: &Ctx, project: &std::path::Path) -> Option<State> {
    seed_dashboard(
        ctx,
        &undashboarded(),
        &[],
        &beside(project, true, &SHOWER_TRUSTED),
    )
    .await
    .map(|wiring| wiring.state)
}

/// How many lists the dashboard was told to show.
fn applied(http: &Fake) -> usize {
    http.requests()
        .iter()
        .filter(|one| one.url.ends_with(APPLY))
        .count()
}

#[tokio::test]
async fn a_dashboard_showing_the_panel_list_already_is_left_as_it_is() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("dashboard-held");
    let probe = contracted_ctx(&project, SHOWER, true, Fake::by_path(Vec::new()));
    let shown = wanted(&probe, &beside(&project, true, &SHOWER_TRUSTED));
    let http = showing_in_turn(vec![Answer::reply(200, shown)], Answer::reply(204, ""));
    let ctx = contracted_ctx(&project, SHOWER, true, http.clone());

    assert_eq!(seeded(&ctx, &project).await, Some(State::AlreadyWired));
    assert_eq!(applied(&http), 0);
}

#[tokio::test]
async fn a_rehearsal_says_what_it_would_show_and_applies_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("dashboard-rehearsed");
    let http = showing_in_turn(vec![Answer::reply(200, NOTHING)], Answer::reply(204, ""));
    let mut ctx = contracted_ctx(&project, SHOWER, true, http.clone());
    ctx.dry_run = true;

    let state = seeded(&ctx, &project).await;

    assert!(
        matches!(&state, Some(State::WouldWire { yours: Some(yours), ours: Some(ours) })
            if yours.starts_with("0 entries") && ours.contains("entries in")),
        "{state:?}"
    );
    assert_eq!(applied(&http), 0);
}

#[tokio::test]
async fn a_search_address_that_is_not_one_fails_naming_the_setting() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("dashboard-searching");
    let http = showing_in_turn(vec![Answer::reply(200, NOTHING)], Answer::reply(204, ""));
    let ctx = contracted_ctx(&project, SHOWER, true, http.clone());
    let settings = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(crate::config::paths::SETTINGS))
        .unwrap_or_default();
    let _ = crate::settings::store::recorded(
        &settings,
        &[(
            crate::settings::own::DASHBOARD_SEARCH,
            Some("duckduckgo.com"),
        )]
        .into(),
    );

    let state = seeded(&ctx, &project).await;

    assert!(
        matches!(&state, Some(State::Failed { detail })
            if detail.starts_with(crate::settings::own::DASHBOARD_SEARCH)),
        "{state:?}"
    );
    assert_eq!(applied(&http), 0);
}

#[tokio::test]
async fn a_stack_running_no_dashboard_is_not_asked() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("dashboard-absent");
    let http = Fake::by_path(Vec::new());
    let ctx = contracted_ctx(&project, SHOWER, true, http.clone());
    let fillers = fillers_at(undashboarded(), &project);

    assert!(seed_dashboard(&ctx, &undashboarded(), &[], &fillers)
        .await
        .is_none());
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn the_dashboard_is_applied_again_over_the_stack_and_what_is_installed() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("dashboard-reshown");
    let http = showing_in_turn(vec![Answer::reply(200, NOTHING)], Answer::reply(204, ""));
    let ctx = contracted_ctx(&project, SHOWER, true, http.clone());
    let stack =
        project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref()).unwrap_or_default();
    let key = crate::plugin::key_file(&stack, SHOWER);
    let _ = std::fs::create_dir_all(key.parent().unwrap_or(&stack));
    let _ = std::fs::write(&key, crate::test_support::CONTRACTED_KEY);
    assert!(crate::config::store::set(
        ctx.settings.env_file.as_deref().unwrap_or(project.as_ref()),
        crate::wiring::FILLS_KEY,
        "dashboard.show=shower",
    )
    .is_ok());
    let kept = crate::app::targets::beside_env(&ctx, crate::config::paths::PLUGINS);
    let mut register = crate::plugin::Register::empty();
    assert!(register
        .record(crate::test_support::contracted(
            SHOWER,
            SHOWER,
            "dashboard.show"
        ))
        .is_ok());
    assert!(crate::app::record::keep(kept.as_deref(), &register).is_ok());

    assert_eq!(reshow(&ctx).await, Some(State::Wired));
    assert_eq!(applied(&http), 1);

    let mut stackless = ctx;
    stackless.stack = crate::test_support::nowhere();
    assert_eq!(reshow(&stackless).await, None);
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

    let state = seed_dashboard(&ctx, &undashboarded(), &[], &fillers)
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
            seed_dashboard(&ctx, &undashboarded(), &[], &fillers)
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

    let state = seed_dashboard(&ctx, &undashboarded(), &[], &fillers)
        .await
        .map(|wiring| wiring.state);

    assert!(
        matches!(&state, Some(State::Failed { detail }) if detail.contains("could not be applied")),
        "{state:?}"
    );
}
