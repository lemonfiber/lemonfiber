//! What the request service trusts for plex.tv in `fronted` mode, written and taken away.

use super::*;
/// Where the request service is handed the authority it trusts for plex.tv, beside `at`.
fn authority(at: &std::path::Path) -> Option<std::path::PathBuf> {
    let fillers = fillers(at);
    let parts = crate::plex_link::fronting(&fillers)?;
    let beneath = crate::plex_link::beneath(&parts)?;
    Some(
        beneath
            .requests
            .join(crate::certificate::fronted::AUTHORITY),
    )
}

/// The state seeding plex.tv's front rests in over `ctx`, where it says anything.
async fn fronting(ctx: &Ctx, at: &std::path::Path) -> Option<State> {
    crate::seed::run::plex_linking::seed_fronting(ctx, &fillers(at))
        .await
        .map(|wiring| wiring.state)
}

#[tokio::test]
async fn fronted_writes_the_authority_once_and_leaving_it_takes_the_authority_away() {
    let http = plex(&[], None, Vec::new());
    let (ctx, at, _) = plex_ctx("plex-fronting", Some("fronted"), Gate::default(), http);
    let mut rehearsing = ctx.clone();
    rehearsing.dry_run = true;
    let trusted = authority(&at).unwrap_or_default();

    assert!(matches!(
        fronting(&rehearsing, &at).await,
        Some(State::WouldWire { .. })
    ));
    assert!(!trusted.exists());
    let first = fronting(&ctx, &at).await;
    assert!(
        matches!(first, Some(State::Wired | State::Failed { .. })),
        "{first:?}"
    );
    assert!(trusted.exists());
    assert_eq!(fronting(&ctx, &at).await, Some(State::AlreadyWired));
    assert_eq!(fronting(&rehearsing, &at).await, Some(State::AlreadyWired));

    assert!(crate::settings::store::recorded(
        &at.join(crate::config::paths::SETTINGS),
        &[(crate::plex_link::SETTING, Some("managed"))].into(),
    )
    .is_ok());
    assert_eq!(fronting(&rehearsing, &at).await, None);
    assert!(trusted.exists());
    assert_eq!(fronting(&ctx, &at).await, None);
    assert!(!trusted.exists());
}

#[test]
fn only_entering_or_leaving_fronted_writes_the_gate_and_the_request_service_anew() {
    let fillers = fillers(stack_root());
    let both = vec![
        crate::app::gating::SERVICE.to_owned(),
        REQUESTS.plugin.to_owned(),
    ];

    assert_eq!(
        crate::plex_link::regenerating(&fillers, None, Some("fronted")),
        both
    );
    assert_eq!(
        crate::plex_link::regenerating(&fillers, Some("fronted"), Some("owner")),
        both
    );
    assert!(crate::plex_link::regenerating(&fillers, None, Some("owner")).is_empty());
}

/// A context over the stack this build carries, written at a scratch directory of its own
/// and recording `mode` as the link mode, running every program as one that succeeds.
fn carried_ctx(
    name: &str,
    mode: &str,
) -> (
    Ctx,
    std::path::PathBuf,
    Arc<lemonfiber_fixtures::support::Recording>,
) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let _ = std::fs::write(at.join(".env"), "");
    assert!(crate::settings::store::recorded(
        &at.join(crate::config::paths::SETTINGS),
        &[(crate::plex_link::SETTING, Some(mode))].into(),
    )
    .is_ok());
    let runner = Arc::new(lemonfiber_fixtures::support::Recording::answering(Ok(
        crate::ports::process::Output {
            status: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        },
    )));
    let mut ctx = a_context()
        .over(crate::test_support::embedded())
        .runner(runner.clone())
        .build();
    ctx.settings.stack_dir = Some(at.clone());
    ctx.settings.env_file = Some(at.join(".env"));
    (ctx, at, runner)
}

#[tokio::test]
async fn files_written_anew_start_the_gate_and_the_request_service_again() {
    let (ctx, at, runner) = carried_ctx("plex-fronting-restarted", "fronted");

    let state = fronting(&ctx, &at).await;

    assert_eq!(state, Some(State::Wired));
    let restarted: Vec<Vec<String>> = runner
        .seen()
        .into_iter()
        .filter(|argv| argv.iter().any(|word| word == "restart"))
        .collect();
    assert_eq!(restarted.len(), 2, "{restarted:?}");
}

#[tokio::test]
async fn files_reached_through_a_link_are_never_read_written_or_taken_away() {
    let http = plex(&[], None, Vec::new());
    let (ctx, at, _) = plex_ctx(
        "plex-fronting-linked",
        Some("fronted"),
        Gate::default(),
        http,
    );
    let trusted = authority(&at).unwrap_or_default();
    let held = trusted
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let elsewhere = at.join("elsewhere");
    let _ = std::fs::create_dir_all(&elsewhere);
    let _ = std::fs::create_dir_all(held.parent().unwrap_or(&at));
    assert!(std::os::unix::fs::symlink(&elsewhere, &held).is_ok());
    let mut rehearsing = ctx.clone();
    rehearsing.dry_run = true;

    let failed = |state: Option<State>| matches!(state, Some(State::Failed { .. }));
    assert!(failed(fronting(&ctx, &at).await));
    assert!(failed(fronting(&rehearsing, &at).await));
    assert!(crate::settings::store::recorded(
        &at.join(crate::config::paths::SETTINGS),
        &[(crate::plex_link::SETTING, Some("managed"))].into(),
    )
    .is_ok());
    assert!(failed(fronting(&ctx, &at).await));
    assert!(std::fs::read_dir(&elsewhere).is_ok_and(|mut entries| entries.next().is_none()));
}
