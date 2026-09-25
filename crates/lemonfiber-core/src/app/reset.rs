//! The full reset — putting the stack back to lemonfiber's own state.
//!
//! Drift is preserved by default: an operator's edit is theirs to keep. A reset is the
//! escape hatch from that — the explicit, confirmed decision to let lemonfiber's state
//! win and discard the edits. Because it discards work, it names exactly what will be
//! lost first, and does nothing until confirmed: unconfirmed it only previews, so the
//! operator sees the diffs before deciding.
//!
//! A reset restores lemonfiber's state *including* the operator's recorded quality choice
//! — that is a managed setting, not drift — so the stack is written carrying the recorded
//! preset, with only the hand-edits on top of it reverted.

use crate::error::{Diagnose, Problem};
use crate::model::ResetReport;

use super::Ctx;

/// Revert the stack to lemonfiber's state, or — until `confirm` — preview what that would
/// discard.
///
/// # Errors
///
/// Returns the [`Problem`] a surface renders when the recorded choice cannot be read or a
/// file cannot be written.
pub(crate) async fn reset(ctx: &Ctx, confirm: bool) -> Result<ResetReport, Box<Problem>> {
    let selection = super::quality::load_selection(ctx)?;
    let record = super::targets::beside_env(ctx, "materialised.json");
    let into = ctx.settings.stack_dir.as_deref();

    let reverted = if confirm {
        super::materialise::reset_stack(
            ctx.stack,
            into,
            record.as_deref(),
            Some(&selection),
            &ctx.settings.unmanaged,
        )
        .map(|(_, edits)| edits)
    } else {
        super::materialise::pending_reverts(
            ctx.stack,
            into,
            record.as_deref(),
            Some(&selection),
            &ctx.settings.unmanaged,
        )
    }
    .map_err(|failure| Box::new(failure.problem()))?;

    // The connections a drifted service value reverts — the download-client categories the
    // operator changed. Read-only unless confirmed, so a preview lists them without writing.
    let reverted_connections = super::seed::reset_connections(ctx, confirm)
        .await
        .into_iter()
        .map(|wiring| wiring.connection)
        .collect();

    Ok(ResetReport {
        reverted,
        reverted_connections,
        confirmed: confirm,
    })
}

#[cfg(test)]
mod tests;
