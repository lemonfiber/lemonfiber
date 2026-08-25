//! Refusing to touch a service database while something may be writing to it.
//!
//! A capture of a live database, or a restore over one, is the corruption a backup
//! exists to prevent — so this fails closed: only a stack *confirmed* stopped goes
//! ahead, and an engine that will not answer is refused as firmly as a running one.
//!
//! Here rather than in the surface that used to ask, because it is the rule and not
//! the wording of it. A surface that had to remember to ask would be a surface that
//! could forget, and the one that forgot would be the one nobody had run yet.

use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::ports::docker::Lifecycle;

use super::Ctx;

/// Whether anything might be writing to a service database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stack {
    /// At least one container is running and may be writing to its database.
    Running,
    /// The engine answered and nothing is running.
    Stopped,
    /// The engine could not be reached, so the stack cannot be confirmed stopped.
    Unknown,
}

/// What the engine says about whether the stack is running.
pub async fn stack(ctx: &Ctx) -> Stack {
    match ctx.engine.list(&ctx.settings.project).await {
        Err(_) => Stack::Unknown,
        Ok(containers) => {
            if containers
                .iter()
                .any(|container| container.lifecycle == Lifecycle::Running)
            {
                Stack::Running
            } else {
                Stack::Stopped
            }
        }
    }
}

/// Let an operation that touches service databases go ahead, or say why not.
///
/// The refusal fails closed: an engine that will not answer leaves lemonfiber
/// unable to prove nothing is writing, and capturing — or restoring over — a
/// database a service is mid-write to is the corruption a backup exists to
/// prevent, so an uncertain answer is refused as firmly as a running one rather
/// than assumed safe.
///
/// The `code` is the caller's, because a capture refused and a restore refused
/// send an operator to different places, and `operation` is the word the sentence
/// is written around.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack is running, or where the engine would not
/// say whether it is.
pub async fn required(ctx: &Ctx, code: Code, operation: &str) -> Result<(), Box<Problem>> {
    match stack(ctx).await {
        Stack::Stopped => Ok(()),
        Stack::Running => Err(Box::new(running(code, operation))),
        Stack::Unknown => Err(Box::new(unproven(code, operation))),
    }
}

/// The refusal for a stack that is up.
fn running(code: Code, operation: &str) -> Problem {
    Problem::new(
        code,
        Severity::Error,
        format!("The stack is running, so a {operation} would not be safe"),
        format!(
            "A {operation} touches the service databases, which must not happen while the \
             services are running and writing to them. Nothing was touched."
        ),
        Remedy::new("Stop the stack first, then try again"),
    )
    .in_state(State::Guided)
}

/// The refusal for a stack nothing can vouch for.
fn unproven(code: Code, operation: &str) -> Problem {
    Problem::new(
        code,
        Severity::Error,
        format!("The stack cannot be confirmed stopped, so a {operation} was not attempted"),
        format!(
            "lemonfiber could not reach the container engine, so it cannot prove nothing is \
             writing to a service database — and will not risk a {operation} over one. Nothing \
             was touched."
        ),
        Remedy::new("Make sure the container engine is running and the stack is down"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lemonfiber_fixtures::support::Reporting;

    use super::{required, stack, Stack};
    use crate::app::Ctx;
    use crate::error::Code;
    use crate::ports::docker::{Engine, Health, Lifecycle};

    /// A code of this module's own, so the tests are about the refusal rather than
    /// about which caller asked for it.
    const ASKED: Code = Code::new("BACKUP-4");

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
}
