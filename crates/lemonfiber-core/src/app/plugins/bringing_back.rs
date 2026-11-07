//! The stack's own services an install was moving in place of, started again when that
//! install is put back.
//!
//! A part moved in place takes its service's Compose name, so starting the plugin
//! recreates that container from the plugin's definition, and taking the plugin back off
//! the machine removes it. Putting the files back restores the stack's own definition and
//! starts nothing, so a running stack would be left without the service.

use std::path::Path;

use crate::plugin::Installed;
use crate::ports::docker::Lifecycle;
use crate::stack::compose::{build, Action};

use super::super::putting_back::Left;
use super::super::Ctx;

/// Start each stack service `would` was moving in place of, where the rest of the stack is
/// running; what could not be started again, said.
pub(super) async fn brought_back(ctx: &Ctx, would: &Installed, stack: &Path) -> Vec<Left> {
    let replaced: Vec<String> = would
        .services
        .iter()
        .filter_map(|placed| placed.replaces.clone())
        .collect();
    if replaced.is_empty() || !stack_running(ctx, would).await {
        return Vec::new();
    }
    let started = match super::super::engine::resolved(ctx, &[]) {
        Ok((_, mut plan)) => {
            plan.services.clone_from(&replaced);
            let start = Action::Start(replaced.clone());
            let command = build(&plan, &ctx.settings, stack, &start, ctx.environment);
            matches!(ctx.seams.runner.run(&command).await, Ok(output) if output.succeeded())
        }
        Err(_) => false,
    };
    if started {
        return Vec::new();
    }
    replaced
        .into_iter()
        .map(|service| Left {
            target: service,
            because: "the stack's own service could not be started again after the install \
                      was put back, so it is not running until the stack is started"
                .to_owned(),
        })
        .collect()
}

/// Whether any of the stack's services other than `would`'s is running.
async fn stack_running(ctx: &Ctx, would: &Installed) -> bool {
    ctx.seams
        .engine
        .list(&ctx.settings.project)
        .await
        .is_ok_and(|containers| {
            containers.iter().any(|container| {
                container.lifecycle == Lifecycle::Running
                    && !would
                        .services
                        .iter()
                        .any(|placed| placed.service == container.service)
            })
        })
}
