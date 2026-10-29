//! One part started or stopped on its own: a part a choice of filler takes out of its
//! form, and one it brings back.

use crate::stack::closure::{Footprint, Plan, Running};
use crate::stack::compose::{build, Action};

use super::super::Ctx;

/// Start `service` of the project in `stack` where `running`, and stop it otherwise,
/// answering why not where it could not be.
///
/// # Errors
///
/// Why, where a start is refused before it runs, or where the engine refused.
pub(crate) async fn ran(
    ctx: &Ctx,
    stack: &std::path::Path,
    service: &str,
    running: bool,
) -> Result<(), String> {
    let named = vec![service.to_owned()];
    let action = if running {
        Action::Start(named.clone())
    } else {
        Action::Stop(named.clone())
    };
    let plan = Plan {
        forms: Vec::new(),
        profiles: ctx
            .stack
            .checked_manifest(ctx.today())
            .ok()
            .zip(crate::app::plugins::read(ctx).ok())
            .and_then(|(manifest, register)| {
                crate::project::profile_of(&manifest, register.installed(), service)
            })
            .into_iter()
            .collect(),
        services: named.clone(),
        dropped: Vec::new(),
        filtered: Vec::new(),
        footprint: Footprint::default(),
        running: Running::Unasked,
    };
    super::before(ctx, &action, &named).map_err(|refused| refused.summary.clone())?;
    let command = build(&plan, &ctx.settings, stack, &action, ctx.environment);
    match ctx.seams.runner.run(&command).await {
        Ok(output) if output.succeeded() => {
            super::halted::after(ctx, &action, &named).await;
            Ok(())
        }
        Ok(refused) => Err(format!(
            "the container engine refused: {}",
            refused.stderr.trim()
        )),
        Err(why) => Err(why.to_string()),
    }
}
