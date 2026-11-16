use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;

use super::{answered, Containers};
use crate::app::Ctx;
use crate::ports::docker::{Health, Lifecycle};
use crate::test_support::media;

/// Where an adapter says whether its upstream answers.
const READY: &str = "/lemonfiber/adapter/v1/ready";

/// A machine reaching its media server moved into its plugin, answering as `http` does,
/// its engine holding `engine`.
fn machine(name: &str, http: Arc<Fake>, engine: Reporting) -> Ctx {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new(name);
    let project = scratch.path().to_path_buf();
    std::mem::forget(scratch);
    let mut ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .build()
        .with_http(http);
    ctx.settings.stack_dir = Some(project.clone());
    ctx.settings.env_file = Some(project.join(".env"));
    let mut ctx = media::reaching(ctx);
    ctx.seams.engine = Arc::new(engine);
    ctx
}

/// How many times an adapter was asked whether its upstream answers.
fn readied(http: &Fake) -> usize {
    http.requests()
        .iter()
        .filter(|request| request.url.ends_with(READY))
        .count()
}

/// What waiting for every service created again since `ctx`'s engine was first read
/// asked of `http`.
async fn waited(ctx: &Ctx, http: &Fake) -> usize {
    let before = Containers::read(ctx).await;
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    if let Some(manifest) = manifest {
        answered(ctx, &manifest, &before).await;
    }
    readied(http)
}

#[tokio::test(start_paused = true)]
async fn a_service_created_again_is_asked_through_its_adapter_until_it_answers() {
    let http = Fake::by_path_in_turn(vec![(
        READY,
        vec![Answer::reply(200, "false"), Answer::reply(200, "true")],
    )]);
    let ctx = machine(
        "recreated-asked",
        http.clone(),
        media::running().recreating_after(1, media::ADAPTER),
    );

    assert_eq!(waited(&ctx, &http).await, 2);
}

#[tokio::test(start_paused = true)]
async fn a_service_created_again_that_answers_at_once_is_asked_once() {
    let http = Fake::by_path(vec![(READY, Answer::reply(200, "true"))]);
    let ctx = machine(
        "recreated-at-once",
        http.clone(),
        media::running().recreating_after(1, media::ADAPTER),
    );

    assert_eq!(waited(&ctx, &http).await, 1);
}

#[tokio::test(start_paused = true)]
async fn a_service_created_again_whose_adapter_publishes_nothing_and_one_kept_are_not_asked() {
    let http = Fake::by_path(vec![(READY, Answer::reply(200, "true"))]);
    let unpublished = machine(
        "recreated-unpublished",
        http.clone(),
        Reporting::holding(&[media::ADAPTER], Lifecycle::Running, Health::Healthy)
            .recreating_after(1, media::ADAPTER),
    );
    assert_eq!(waited(&unpublished, &http).await, 0);

    let kept = machine("recreated-none", http.clone(), media::running());
    assert_eq!(waited(&kept, &http).await, 0);
}
