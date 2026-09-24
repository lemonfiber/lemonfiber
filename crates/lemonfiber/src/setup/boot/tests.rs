use super::{afterwards, condition, gating, overall, preflight, start};
use crate::exit::{shown, success};
use lemonfiber_core::app::Outcome;
use lemonfiber_core::config::Protocols;
use lemonfiber_core::doctor::{autostart, Category, Finding, Overall, Verdict};
use lemonfiber_core::error::Remedy;
use lemonfiber_core::stack::Source;

use crate::setup::tests::{ctx, working_ctx, FakeEngine, Scripted};

#[tokio::test]
async fn an_environment_that_cannot_work_stops_setup_before_a_question() {
    // Nothing setup does works without a container engine, so it is checked
    // before the first question rather than after eleven answers.
    assert!(preflight(&ctx()).await.is_err());
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_stops_setup_with_its_own_words() {
    // The checks need the stack before any of them can run, so a stack that
    // will not read is reported as itself rather than as a failed environment.
    let mut ctx = working_ctx();
    ctx.stack = Source::External(std::path::Path::new("/lemonfiber-not-a-real-stack"));
    assert!(preflight(&ctx).await.is_err());
}

#[tokio::test]
async fn an_environment_that_works_passes_without_a_word() {
    assert!(preflight(&working_ctx()).await.is_ok());
}

/// Autostart it could not confirm does not stop somebody setting up.
///
/// The check that answers it is in this very family and answers
/// `enabled-unverified` wherever Docker Desktop's own setting cannot be read —
/// which is most machines, and says nothing about whether this one can run the
/// stack today. An undetermined finding stops setup, so left in this would have
/// meant that anybody who had answered yes to starting on boot could not run
/// setup a second time.
#[test]
fn a_prerequisite_nobody_could_confirm_does_not_stop_setup() {
    let unverified = Finding::in_category(
        Category::Environment,
        autostart::CHECK,
        "The stack comes back after a restart",
        Verdict::Unverified {
            reason: "enabled-unverified".to_owned(),
            remedy: Remedy::new("Open Docker Desktop"),
        },
    );
    let engine = Finding::in_category(
        Category::Environment,
        "environment.engine",
        "Docker engine",
        Verdict::Pass { note: None },
    );

    let kept = gating(vec![engine, unverified]);

    assert_eq!(kept.len(), 1, "the autostart finding is the one dropped");
    assert_eq!(overall(&kept), Overall::Healthy, "so setup goes on");
}

#[tokio::test]
async fn a_pull_that_failed_stops_before_starting_against_images_that_never_came() {
    // Starting against images that never arrived is worse than not starting.
    let code = start(&ctx(), &Scripted::saying(false, &[])).await;
    assert_ne!(shown(code), success());
}

/// A stack with no services of its own — enough for a real `up` to run and
/// settle, without the fifteen containers the shipped one would wait on.
static QUIET: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/quiet-stack");

#[test]
fn only_a_lifecycle_says_what_the_stack_settled_to() {
    // Asked of whatever `up` handed back: anything that is not a lifecycle report has
    // no condition to read, and offering a walk over one would be a guess.
    let report = lemonfiber_core::model::VersionReport {
        binary: "0".to_owned(),
        supported_schema: Vec::new(),
        stack: String::new(),
        compose: None,
        changelog: lemonfiber_core::changelog::Notes::unread(),
    };
    assert_eq!(condition(&Outcome::Version(report)), None);
}

#[tokio::test]
async fn a_first_run_ends_by_saying_what_to_send_the_people_who_live_here() {
    // The whole of what setup adds once the stack is up, read back as one value:
    // the cost this stack was decided at, and then the address, which is the
    // question setup is about to be asked and the one nothing above it answers.
    let mut ctx = working_ctx();
    ctx.engine = std::sync::Arc::new(FakeEngine::quiet());
    ctx.settings.protocols = Protocols::both();

    let said = afterwards(&ctx).await.join("\n");

    assert!(said.contains("No port is forwarded"), "{said}");
    assert!(said.contains("front door"), "{said}");
    let cost = said.find("No port is forwarded");
    let door = said.find("front door");
    assert!(
        cost.is_some_and(|cost| door.is_some_and(|door| door > cost)),
        "the address is the last thing on the screen: {said}"
    );
}

#[tokio::test]
async fn a_stack_that_came_up_reports_how_it_settled() {
    // The far end of a first run: images down, stack up, and how it settled put
    // on screen. It needs somewhere to write the stack Docker reads and an
    // engine that answers, which is what an applied setup leaves behind.
    let stack_dir =
        std::env::temp_dir().join(format!("lemonfiber-boot-{}-started", std::process::id()));
    let _ = std::fs::remove_dir_all(&stack_dir);
    let mut ctx = working_ctx();
    ctx.engine = std::sync::Arc::new(FakeEngine::quiet());
    ctx.stack = Source::Embedded(&QUIET);
    ctx.settings.protocols = Protocols::both();
    ctx.settings.stack_dir = Some(stack_dir.clone());

    let code = start(&ctx, &Scripted::saying(false, &[])).await;

    assert_eq!(
        shown(code),
        success(),
        "a stack that came up is not reported as a failure"
    );
    let _ = std::fs::remove_dir_all(&stack_dir);
}
