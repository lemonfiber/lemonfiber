use std::sync::Arc;

use lemonfiber_fixtures::support::Reporting;

use super::{required, stack, Stack};
use crate::app::Ctx;
use crate::error::Code;
use crate::ports::docker::{Engine, Health, Lifecycle};

/// A code of this module's own, so the tests are about the refusal rather than
/// about which caller asked for it.
const ASKED: Code = crate::error::codes::backup::STILL_RUNNING;

fn ctx(engine: Arc<dyn Engine>) -> Ctx {
    crate::test_support::a_context().engine(engine).build()
}

/// An engine that answers, holding one service in the given state.
fn engine(lifecycle: Lifecycle) -> Arc<dyn Engine> {
    Arc::new(Reporting::holding(&["sonarr"], lifecycle, Health::None))
}

#[tokio::test]
async fn a_stack_with_nothing_running_may_be_touched() {
    let ctx = ctx(engine(Lifecycle::Exited));
    assert_eq!(stack(&ctx).await, Stack::Stopped);
    assert!(required(&ctx, ASKED, "backup").await.is_ok());
}

#[tokio::test]
async fn a_running_stack_is_refused_and_told_to_stop_first() {
    let ctx = ctx(engine(Lifecycle::Running));
    assert_eq!(stack(&ctx).await, Stack::Running);
    let refusal = required(&ctx, ASKED, "backup")
        .await
        .err()
        .map(|problem| (problem.code, problem.summary.clone()));
    assert_eq!(
        refusal,
        Some((
            ASKED,
            "The stack is running, so a backup would not be safe".to_owned()
        ))
    );
}

#[tokio::test]
async fn an_engine_that_will_not_answer_is_refused_as_firmly_as_a_running_stack() {
    // Failing closed: unable to prove nothing is writing is not the same as
    // knowing nothing is, and only one of the two is safe to act on.
    let ctx = ctx(Arc::new(Reporting::absent()));
    assert_eq!(stack(&ctx).await, Stack::Unknown);
    let refusal = required(&ctx, ASKED, "restore")
        .await
        .err()
        .map(|problem| (problem.code, problem.summary.clone()));
    assert_eq!(
        refusal,
        Some((
            ASKED,
            "The stack cannot be confirmed stopped, so a restore was not attempted".to_owned()
        ))
    );
}
