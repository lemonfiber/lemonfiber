//! Whether the quality sync's configuration was edited by hand, and putting the preset
//! back over that edit, over `quality.sync`.

use crate::app::Ctx;
use crate::error::codes::qual::UNREAPPLIED;
use crate::error::{Problem, Remedy, State};
use crate::model::StackEdit;
use crate::quality::Selection;

/// Whether the quality sync's configuration holds a hand edit that putting `selection`
/// back would replace: never where no sync speaks the contract or it does not answer.
pub(super) async fn customised(ctx: &Ctx, selection: &Selection) -> bool {
    asked(&ctx.clone().rehearsing(), selection)
        .await
        .is_some_and(|edited| edited.is_ok_and(|edit| edit.is_some()))
}

/// Put `selection` back over a hand edit, or on a rehearsal say what that replaces: the
/// edit, or nothing where the configuration is lemonfiber's own or no sync speaks the
/// contract.
///
/// # Errors
///
/// Returns a [`Problem`] where the sync would not answer.
pub(super) async fn reapplied(
    ctx: &Ctx,
    selection: &Selection,
) -> Result<Option<StackEdit>, Box<Problem>> {
    match asked(ctx, selection).await {
        Some(Ok(edit)) => Ok(edit),
        Some(Err(why)) => Err(unreapplied(&why)),
        None => Ok(None),
    }
}

/// What every quality sync speaking `quality.sync` answered, or nothing where none does,
/// the operator declared it unmanaged, or the stack cannot be read.
async fn asked(ctx: &Ctx, selection: &Selection) -> Option<Result<Option<StackEdit>, String>> {
    let (_, _, fillers) = crate::seed::run::managed(ctx)?;
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
