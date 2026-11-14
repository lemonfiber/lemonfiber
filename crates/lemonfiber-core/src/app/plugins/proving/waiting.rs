//! A plugin in a profile of its own, stopped where none of the forms it joins is running.

use std::path::Path;

use crate::plugin::Installed;
use crate::stack::compose::Action;

use super::super::super::Ctx;
use super::invocation;

/// Stop the containers of a plugin sitting in a profile of its own, leaving them on the
/// machine, where none of the forms it joins is running: what it waits for, or nothing
/// where the bundle places it, one of its forms is running, it joins every form, or it
/// would not stop.
pub(crate) async fn waiting(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    installed: &Installed,
    stack: &Path,
) -> Vec<String> {
    let own = crate::plugin::profile_of(installed) == crate::plugin::profile(&installed.plugin);
    if !own || installed.forms.is_empty() {
        return Vec::new();
    }
    let running = super::super::super::engine::active(ctx, manifest).await;
    if installed.forms.iter().any(|form| running.contains(form)) {
        return Vec::new();
    }
    let services = installed
        .services
        .iter()
        .map(|placed| placed.service.clone())
        .collect();
    let command = invocation(ctx, installed, stack, &Action::Stop(services));
    if matches!(ctx.seams.runner.run(&command).await, Ok(output) if output.succeeded()) {
        installed.forms.clone()
    } else {
        Vec::new()
    }
}
