//! What naming a form would come to, asked without starting anything.
//!
//! Apart from the lifecycle commands beside it because it is the one resolution
//! that asks the engine something as well: which of the services a start would
//! reach are already running.

use super::resolved;
use crate::app::Ctx;
use crate::error::Problem;
use crate::ports::docker::Lifecycle;
use crate::stack::closure::{Plan, Running};

/// What naming these forms would come to, without running anything.
///
/// The same resolution a lifecycle command does, stopping where it would start
/// spawning Compose — so what this answers and what that does cannot disagree
/// about which services a form holds or why one was left out. A surface states
/// it before acting; an operator can also just ask.
///
/// It also names, among the services it would start, those already running, read
/// from the engine once. An engine that will not answer is said as such rather than
/// as nothing running.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read
/// or the forms cannot be resolved — an unknown name among them, a form that
/// refuses company, or a closure the configuration empties.
pub(crate) async fn preview(ctx: &Ctx, forms: &[String]) -> Result<Plan, Box<Problem>> {
    let (_, mut plan) = resolved(ctx, forms)?;
    plan.running = already_running(ctx, &plan.services).await;
    Ok(plan)
}

/// Which of `services` the engine reports as running, in their order, or that the
/// engine would not say.
async fn already_running(ctx: &Ctx, services: &[String]) -> Running {
    let Ok(containers) = ctx.seams.engine.list(&ctx.settings.project).await else {
        return Running::Unread;
    };
    Running::Read(
        services
            .iter()
            .filter(|service| {
                containers.iter().any(|container| {
                    container.service == **service && container.lifecycle == Lifecycle::Running
                })
            })
            .cloned()
            .collect(),
    )
}
