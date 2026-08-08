//! Refusing to touch a service database while something may be writing to it.
//!
//! A capture of a live database, or a restore over one, is the corruption a backup
//! exists to prevent — so this fails closed: only a stack *confirmed* stopped goes
//! ahead, and an engine that will not answer is refused as firmly as a running one.

use std::process::ExitCode;

use lemonfiber_core::app::Ctx;
use lemonfiber_core::backup::Relocation;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{self, store};
use lemonfiber_core::error::Diagnose;
use lemonfiber_core::ports::docker::{Container, Failure, Lifecycle};

use super::Stack;
use crate::render::Lines;

/// Ask the engine whether the stack is running.
pub(super) async fn stack_state(ctx: &Ctx) -> Stack {
    stack_from(ctx.engine.list(&ctx.settings.project).await)
}

/// What a container listing means for whether anything may be writing to a database.
pub(super) fn stack_from(listed: Result<Vec<Container>, Failure>) -> Stack {
    match listed {
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
        Err(_) => Stack::Unknown,
    }
}

/// Refuse an operation that touches service databases unless the stack is
/// *confirmed* stopped, returning the exit code to end on where it is not.
///
/// The refusal fails closed: an engine that will not answer leaves it unable to
/// prove nothing is writing, and capturing — or restoring over — a database a
/// service is mid-write to is the corruption a backup exists to prevent, so an
/// uncertain answer is refused as firmly as a running one rather than assumed safe.
pub(super) async fn require_stopped(ctx: &Ctx, operation: &str) -> Option<ExitCode> {
    let refusal = refuse(&stack_state(ctx).await, operation)?;
    refusal.eprint();
    Some(ExitCode::FAILURE)
}

/// What to tell the operator about a state that will not permit the operation, or
/// nothing at all where it may go ahead.
pub(super) fn refuse(stack: &Stack, operation: &str) -> Option<Lines> {
    let mut lines = Lines::default();
    match stack {
        Stack::Stopped => return None,
        Stack::Running => {
            lines.put(format!(
                "A {operation} touches the service databases, which must not happen while the \
                 services are running and writing to them."
            ));
            lines.put("Stop the stack first:  lemonfiber down <form>");
        }
        Stack::Unknown => {
            lines.put(format!(
                "lemonfiber could not reach the container engine, so it cannot confirm the stack \
                 is stopped — and will not risk a {operation} over a database a service may be \
                 writing."
            ));
            lines.put("Make sure Docker is running and the stack is down, then try again.");
        }
    }
    Some(lines)
}

/// Point the restored environment file at this machine's data root, or the code to
/// end on where it will not take it.
///
/// The file that landed still names the root the backup was taken against, which is
/// not on this machine; this is the adjustment the re-point offered, applied once
/// the files are in place.
pub(super) fn repoint_env(paths: &Paths, relocation: &Relocation) -> Option<ExitCode> {
    let failure = store::set(&paths.env_file(), config::DATA_ROOT_KEY, &relocation.now).err()?;
    Some(crate::complain(&failure.problem()))
}

/// A timestamp for a backup — seconds since the epoch, filename-safe and sorting
/// by age, or empty on a clock too absurd to read.
pub(super) fn stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs().to_string())
        .unwrap_or_default()
}
