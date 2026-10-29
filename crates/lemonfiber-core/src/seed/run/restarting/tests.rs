use std::sync::Arc;

use lemonfiber_contract::Contracted;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Recording;

use super::Restarting;
use crate::app::Ctx;
use crate::journal::Kind;
use crate::ports::process::Output;
use crate::ports::service::{Applied, Failure};
use crate::seed::State;
use crate::test_support::media;

/// A machine with the media server moved into its plugin, its stack written at a project
/// of its own, running programs as `runner` does.
fn machine(name: &str, runner: Arc<Recording>) -> Ctx {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new(name);
    let project = scratch.path().to_path_buf();
    std::mem::forget(scratch);
    let mut ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .runner(runner)
        .build();
    ctx.settings.stack_dir = Some(project.clone());
    ctx.settings.env_file = Some(project.join(".env"));
    media::reaching(ctx)
}

fn ran() -> Arc<Recording> {
    Arc::new(Recording::answering(Ok(Output {
        status: Some(0),
        stdout: String::new(),
        stderr: String::new(),
    })))
}

/// The media server's adapter, saying its upstream answers, as what restarts it.
fn restarting() -> Restarting {
    Restarting {
        adapter: Contracted::new(
            Fake::by_path(vec![(
                "/lemonfiber/adapter/v1/ready",
                Answer::reply(200, "true"),
            )]),
            "http://jellyfin-adapter:8080",
            "jellyfin-adapter",
            "k3y",
        ),
        upstream: "jellyfin".to_owned(),
    }
}

fn restarts(runner: &Recording) -> usize {
    runner
        .seen()
        .iter()
        .filter(|argv| argv.iter().any(|word| word == "restart"))
        .count()
}

fn journalled(ctx: &Ctx) -> Vec<Kind> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .map(|change| change.kind.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(start_paused = true)]
async fn what_is_in_force_at_once_is_wired_and_a_failure_is_unreached() {
    let runner = ran();
    let ctx = machine("restarting-now", runner.clone());
    let restarting = restarting();

    assert_eq!(
        restarting.taken_up(&ctx, Ok(Applied::Now)).await,
        State::Wired
    );
    let unreached = || Failure::Unavailable {
        service: "jellyfin-adapter".to_owned(),
    };
    assert_eq!(
        restarting.taken_up(&ctx, Err(unreached())).await,
        crate::seed::unreached(&unreached())
    );
    assert_eq!(restarts(&runner), 0);
}

#[tokio::test(start_paused = true)]
async fn what_is_in_force_once_it_restarts_restarts_the_upstream_journalled_first() {
    let runner = ran();
    let ctx = machine("restarting-restart", runner.clone());

    let state = restarting().taken_up(&ctx, Ok(Applied::OnRestart)).await;

    assert_eq!(
        state,
        State::Restarted {
            service: "jellyfin".to_owned()
        }
    );
    assert_eq!(restarts(&runner), 1);
    assert!(journalled(&ctx).contains(&Kind::Restarted {
        service: "jellyfin".to_owned()
    }));
}

#[tokio::test(start_paused = true)]
async fn a_restart_nothing_can_journal_is_not_made() {
    let runner = ran();
    let mut ctx = machine("restarting-unjournalled", runner.clone());
    ctx.settings.env_file = None;

    let state = restarting().taken_up(&ctx, Ok(Applied::OnRestart)).await;

    assert!(
        matches!(&state, State::Failed { detail }
            if detail.starts_with("jellyfin takes it up only once it starts again")
                && detail.contains("there is nowhere to journal the restart")),
        "{state:?}"
    );
    assert_eq!(restarts(&runner), 0);

    let ctx = machine("restarting-unwritable", runner.clone());
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    let _ = std::fs::create_dir_all(&journal);

    let state = restarting().taken_up(&ctx, Ok(Applied::OnRestart)).await;

    assert!(
        matches!(&state, State::Failed { detail } if detail.contains("could not be journalled")),
        "{state:?}"
    );
    assert_eq!(restarts(&runner), 0);
}

#[test]
fn an_adapter_restarts_the_upstream_it_fronts_and_a_service_fronting_none_itself() {
    let mut adapter = crate::test_support::a_placed("server-adapter", &["media.serve"], Some(8080));
    adapter.speaks = vec!["media.serve@1".to_owned()];
    adapter.fronts = Some("server".to_owned());
    let upstream = crate::test_support::a_placed("server", &[], Some(8096));
    let installed = [crate::test_support::an_installed(
        "serving",
        vec![adapter, upstream],
    )];
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK).ok();
    let fillers = manifest.map(|manifest| {
        crate::wiring::Fillers::of(
            &manifest,
            &installed,
            &crate::wiring::Chosen::default(),
            None,
        )
    });
    let contracted = || Contracted::new(Fake::by_path(Vec::new()), "http://a", "a", "k");
    let upstream_of = |id: &str| {
        fillers.as_ref().and_then(|fillers| {
            fillers
                .service(id)
                .map(|asker| Restarting::of(fillers, asker, contracted()).upstream)
        })
    };

    assert_eq!(upstream_of("server-adapter").as_deref(), Some("server"));
    assert_eq!(upstream_of("server").as_deref(), Some("server"));
}
