use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Recording;

use super::{made_before_start, pins, seed_internal_tls};
use crate::app::targets::MediaServer;
use crate::app::Ctx;
use crate::certificate::internal;
use crate::ports::process::Output;
use crate::seed::State;
use crate::test_support::media;

/// A machine with the media server moved into its plugin, its stack written at a project
/// of its own, answering as `http` does and running programs as `runner` does.
fn machine(name: &str, http: Arc<Fake>, runner: Arc<Recording>) -> (Ctx, std::path::PathBuf) {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new(name);
    let project = scratch.path().to_path_buf();
    std::mem::forget(scratch);
    let mut ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .runner(runner)
        .build()
        .with_http(http);
    ctx.settings.stack_dir = Some(project.clone());
    ctx.settings.env_file = Some(project.join(".env"));
    let ctx = media::reaching(ctx);
    (ctx, project)
}

/// The media server as the stack written at `project` holds it.
fn server(ctx: &Ctx) -> Option<MediaServer> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    crate::app::targets::hosted(ctx, &manifest)
}

fn ran(status: i32) -> Arc<Recording> {
    Arc::new(Recording::answering(Ok(Output {
        status: Some(status),
        stdout: String::new(),
        stderr: "no such service".to_owned(),
    })))
}

/// A media server answering `serve_tls` with `answer`, and its declaration.
fn serving(answer: Answer) -> Arc<Fake> {
    Fake::by_path(vec![
        (media::serve("serve_tls"), answer),
        (
            media::serve("declared"),
            media::answered(&lemonfiber_fixtures::declaring::enforceable()),
        ),
    ])
}

/// Where `serve_tls` was asked to serve from, each time it was asked.
fn handed(http: &Fake) -> Vec<serde_json::Value> {
    media::served(&http.requests(), "serve_tls")
        .into_iter()
        .filter_map(|asked| asked.get("path").cloned())
        .collect()
}

#[tokio::test]
async fn the_certificate_is_made_before_the_stack_starts_and_is_what_holders_are_handed() {
    let (ctx, project) = machine("encrypting-made", serving(media::taken()), ran(0));
    let server = server(&ctx);
    let Some(server) = server else {
        unreachable!("the moved media server is hosted");
    };
    assert_eq!(pins(&server), Vec::<String>::new(), "none is made yet");

    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let Some(manifest) = manifest else {
        unreachable!("the stack reads");
    };
    made_before_start(&ctx, &manifest);
    let config = server.served.confined_to.clone().unwrap_or_default();
    assert!(config.starts_with(&project), "{}", config.display());
    let kept = internal::kept(&config).ok().flatten();
    assert!(kept.is_some(), "{}", config.display());
    assert_eq!(
        pins(&server),
        kept.map(|kept| vec![kept.fingerprint]).unwrap_or_default()
    );

    let again = internal::kept(&config).ok().flatten();
    made_before_start(&ctx, &manifest);
    assert_eq!(
        internal::kept(&config).ok().flatten(),
        again,
        "it is made once"
    );
}

#[tokio::test(start_paused = true)]
async fn a_server_serving_it_already_is_left_running() {
    let http = serving(media::taken());
    let runner = ran(0);
    let (ctx, _) = machine("encrypting-now", http.clone(), runner.clone());
    let wiring = seed_internal_tls(&ctx, server(&ctx).as_ref()).await;
    assert_eq!(wiring.map(|one| one.state), Some(State::AlreadyWired));
    assert_eq!(
        handed(&http),
        vec![serde_json::json!("/config/lemonfiber/internal.p12")]
    );
    assert!(!runner.ran("restart"));
}

#[tokio::test(start_paused = true)]
async fn a_server_serving_it_only_once_it_restarts_is_started_again_and_waited_for() {
    let http = serving(Answer::reply(205, ""));
    let runner = ran(0);
    let (ctx, _) = machine("encrypting-restart", http.clone(), runner.clone());
    let wiring = seed_internal_tls(&ctx, server(&ctx).as_ref()).await;
    assert_eq!(wiring.map(|one| one.state), Some(State::Wired));
    let restarted: Vec<Vec<String>> = runner
        .seen()
        .into_iter()
        .filter(|argv| argv.iter().any(|word| word == "restart"))
        .collect();
    assert_eq!(restarted.len(), 1, "{restarted:?}");
    assert!(restarted
        .first()
        .is_some_and(|argv| argv.last().map(String::as_str) == Some("jellyfin")));
    assert!(http.asked_for(media::serve("declared")));

    let failing = ran(1);
    let (ctx, _) = machine(
        "encrypting-restart-failed",
        serving(Answer::reply(205, "")),
        failing,
    );
    let wiring = seed_internal_tls(&ctx, server(&ctx).as_ref()).await;
    assert!(matches!(
        wiring.map(|one| one.state),
        Some(State::Failed { detail }) if detail.contains("no such service")
    ),);
}

#[tokio::test(start_paused = true)]
async fn a_server_serving_no_internal_port_is_unmatched_and_a_rehearsal_writes_nothing() {
    let unsupported = serving(Answer::served(
        500,
        crate::ports::http::PROBLEM,
        r#"{"type": "unsupported", "detail": "no internal port"}"#,
    ));
    let (ctx, _) = machine("encrypting-unsupported", unsupported, ran(0));
    let wiring = seed_internal_tls(&ctx, server(&ctx).as_ref()).await;
    assert!(matches!(
        wiring.map(|one| one.state),
        Some(State::Unmatched { .. })
    ));

    let http = serving(media::taken());
    let (mut ctx, _) = machine("encrypting-rehearsed", http.clone(), ran(0));
    ctx.dry_run = true;
    let wiring = seed_internal_tls(&ctx, server(&ctx).as_ref()).await;
    assert!(matches!(
        wiring.map(|one| one.state),
        Some(State::WouldWire { ours: Some(path), .. }) if path.ends_with("lemonfiber/internal.p12")
    ));
    assert!(handed(&http).is_empty());
}

#[tokio::test(start_paused = true)]
async fn without_an_administrator_or_a_media_server_nothing_is_asked() {
    let http = serving(media::taken());
    let scratch = lemonfiber_fixtures::scratch::Scratch::new("encrypting-unadministered");
    let mut ctx = crate::test_support::a_context()
        .build()
        .with_http(http.clone());
    ctx.settings.stack_dir = Some(scratch.path().to_path_buf());
    ctx.settings.env_file = Some(scratch.path().join(".env"));
    media::moved_in(&ctx);
    let found = server(&ctx);
    assert!(found.is_some(), "the moved media server is hosted");
    assert_eq!(seed_internal_tls(&ctx, found.as_ref()).await, None);
    ctx.dry_run = true;
    assert!(matches!(
        seed_internal_tls(&ctx, found.as_ref())
            .await
            .map(|one| one.state),
        Some(State::WouldWire { yours: None, .. })
    ));
    assert_eq!(seed_internal_tls(&ctx, None).await, None);
    let bundled = found.map(|mut server| {
        server.served.config_at = None;
        server
    });
    assert_eq!(seed_internal_tls(&ctx, bundled.as_ref()).await, None);
    assert!(handed(&http).is_empty());
}
