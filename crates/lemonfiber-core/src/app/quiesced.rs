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

/// What the engine says about whether lemonfiber's own stack is running.
pub async fn stack(ctx: &Ctx) -> Stack {
    of(ctx, &ctx.settings.project).await
}

/// What the engine says about whether a named Compose project is running.
///
/// Separate from [`stack`] because the project that must be still is not always
/// lemonfiber's. A capture taken before a takeover covers the setup that is already
/// here, and it is *that* project's containers which might be mid-write to the
/// databases being copied; asking about lemonfiber's own would be asking about a
/// project that does not exist yet, and an engine that truthfully reports nothing
/// running under a name nothing uses would answer `Stopped` every time — a proof
/// that always passes, which is no proof at all.
pub async fn of(ctx: &Ctx, project: &str) -> Stack {
    match ctx.engine.list(project).await {
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
    answered(stack(ctx).await, code, operation)
}

/// The same, for a named Compose project rather than lemonfiber's own.
///
/// # Errors
///
/// Returns a [`Problem`] where that project is running, or where the engine would
/// not say whether it is.
pub(crate) async fn required_of(
    ctx: &Ctx,
    project: &str,
    code: Code,
    operation: &str,
) -> Result<(), Box<Problem>> {
    answered(of(ctx, project).await, code, operation)
}

/// Turn what the engine said into the answer the caller gets.
///
/// One place rather than two, so the project-scoped proof and lemonfiber's own
/// cannot come to differ about what counts as proven — and in particular so an
/// engine that will not answer stays a refusal in both.
fn answered(stack: Stack, code: Code, operation: &str) -> Result<(), Box<Problem>> {
    match stack {
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
mod tests;
