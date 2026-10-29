//! Whether the quality sync's configuration was edited by hand, and putting the preset
//! back over that edit: over `quality.sync` where a sync speaks it, and in process where
//! none does.

use std::path::Path;

use crate::app::materialise::{quality_sync_customised, reapply_preset, QUALITY_SYNC_CONFIG};
use crate::app::Ctx;
use crate::error::codes::qual::UNREAPPLIED;
use crate::error::{Diagnose as _, Problem, Remedy, State};
use crate::model::StackEdit;
use crate::ports::service::Edited;
use crate::quality::Selection;

/// Whether the quality sync's configuration holds a hand edit that putting `selection`
/// back would replace; recorded against `record` where no sync speaks the contract.
pub(super) async fn customised(ctx: &Ctx, selection: &Selection, record: Option<&Path>) -> bool {
    match asked(&ctx.clone().rehearsing(), selection).await {
        Some(edited) => edited.is_ok_and(|edit| edit.is_some()),
        None => quality_sync_customised(
            ctx.seams.confined.as_ref(),
            ctx.settings.stack_dir.as_deref(),
            record,
        ),
    }
}

/// Put `selection` back over a hand edit, or on a rehearsal say what that replaces: the
/// edit, or nothing where the configuration is lemonfiber's own or the operator declared
/// it theirs.
///
/// # Errors
///
/// Returns a [`Problem`] where the sync would not answer or the configuration could not
/// be written.
pub(super) async fn reapplied(
    ctx: &Ctx,
    selection: &Selection,
    record: Option<&Path>,
) -> Result<Option<StackEdit>, Box<Problem>> {
    if crate::unmanaged::covers(&ctx.settings.unmanaged, QUALITY_SYNC_CONFIG) {
        return Ok(None);
    }
    match asked(ctx, selection).await {
        Some(Ok(edit)) => Ok(edit.map(|Edited { path, diff }| StackEdit { path, diff })),
        Some(Err(why)) => Err(unreapplied(&why)),
        None => reapply_preset(
            ctx.seams.confined.as_ref(),
            ctx.stack,
            ctx.settings.stack_dir.as_deref(),
            record,
            selection,
            &ctx.settings.unmanaged,
            ctx.dry_run,
        )
        .map_err(|failure| Box::new(failure.problem())),
    }
}

/// What every quality sync speaking `quality.sync` answered, or nothing where none does or
/// the stack cannot be read.
async fn asked(ctx: &Ctx, selection: &Selection) -> Option<Result<Option<Edited>, String>> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let fillers = crate::app::targets::host_fillers(ctx, &manifest, project.as_deref());
    crate::seed::run::reapplied(ctx, &fillers, selection).await
}

/// The refusal of a sync that would not show or put back the preset, with why.
fn unreapplied(why: &str) -> Box<Problem> {
    let declared = UNREAPPLIED.declaration();
    Box::new(
        Problem::new(
            UNREAPPLIED,
            format!("The quality sync did not put the preset back: {why}"),
            declared.map_or("", |declared| declared.meaning()),
            Remedy::new(declared.map_or("", |declared| declared.remedy())),
        )
        .in_state(State::Guided),
    )
}

#[cfg(test)]
mod tests;
