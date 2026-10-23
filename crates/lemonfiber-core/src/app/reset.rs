//! The full reset — putting the stack back to lemonfiber's own state.
//!
//! Drift is preserved by default: an operator's edit is theirs to keep. A reset is the
//! escape hatch from that — the explicit, confirmed decision to let lemonfiber's state
//! win and discard the edits. Because it discards work, it names exactly what will be
//! lost first, and does nothing until the offer that reading made is answered; the reset
//! reads it again when it runs and refuses where anything moved.
//!
//! A reset restores lemonfiber's state *including* the operator's recorded quality choice
//! — that is a managed setting, not drift — so the stack is written carrying the recorded
//! preset, with only the hand-edits on top of it reverted.

use crate::error::codes::stack::RESET_MOVED;
use crate::error::{Diagnose, Problem, Remedy, State};
use crate::model::{ResetReport, StackEdit};

use super::Ctx;

/// The parts a reset's offer is named over, in the order it is built.
const OFFERED: [&str; 2] = ["the edits it reverts", "the connections it reverts"];

/// Revert the stack to lemonfiber's state, or — until its offer is answered — preview what
/// that would discard.
///
/// # Errors
///
/// Returns the [`Problem`] a surface renders when the recorded choice or the record of
/// installed plugins cannot be read, a file cannot be written, or the offer answered is
/// not the one a reading makes now.
pub(crate) async fn reset(
    ctx: &Ctx,
    agreement: Option<String>,
) -> Result<ResetReport, Box<Problem>> {
    let (reverted, reverted_connections) = reading(ctx, false).await?;
    let standing = offer(&reverted, &reverted_connections);
    let Some(answered) = agreement else {
        return Ok(ResetReport {
            rehearsed: false,
            reverted,
            reverted_connections,
            confirmed: false,
            offer: standing,
        });
    };
    let moved = crate::agreement::differs(&answered, &standing, &OFFERED);
    if !moved.is_empty() {
        return Err(Box::new(reset_moved(&moved, &standing)));
    }
    let (reverted, reverted_connections) = reading(ctx, true).await?;
    Ok(ResetReport {
        rehearsed: false,
        reverted,
        reverted_connections,
        confirmed: true,
        offer: standing,
    })
}

/// The offer a reading of a reset makes: each edit it reverts, then each connection.
fn offer(reverted: &[StackEdit], connections: &[String]) -> String {
    let edits: Vec<&str> = reverted
        .iter()
        .flat_map(|edit| [edit.path.as_str(), edit.diff.as_str()])
        .collect();
    let connections: Vec<&str> = connections.iter().map(String::as_str).collect();
    crate::agreement::parted(&[&edits, &connections])
}

/// A reset answering a reading of it that has since moved.
fn reset_moved(moved: &[&str], standing: &str) -> Problem {
    Problem::new(
        RESET_MOVED,
        "That reset was agreed to against a different reading of what it would revert",
        format!(
            "Since it was read, {} changed, so nothing was reverted. Agreeing to it now would \
             be agreeing to lose something nobody read.",
            moved.join(" and ")
        ),
        Remedy::new("Read what a reset would revert again, and answer the name it prints")
            .with_detail(format!("the offer standing now is {standing}")),
    )
    .in_state(State::Guided)
}

/// What a reset reverts, carried out where `confirm` and only read otherwise.
async fn reading(ctx: &Ctx, confirm: bool) -> Result<(Vec<StackEdit>, Vec<String>), Box<Problem>> {
    let selection = super::quality::load_selection(ctx)?;
    let record = super::targets::beside_env(ctx, "materialised.json");
    let into = ctx.settings.stack_dir.as_deref();

    let mut reverted = if confirm {
        super::materialise::reset_stack(
            ctx.seams.confined.as_ref(),
            ctx.stack,
            into,
            record.as_deref(),
            Some(&selection),
            &ctx.settings.unmanaged,
        )
        .map(|(_, edits)| edits)
    } else {
        super::materialise::pending_reverts(
            ctx.seams.confined.as_ref(),
            ctx.stack,
            into,
            record.as_deref(),
            Some(&selection),
            &ctx.settings.unmanaged,
        )
    }
    .map_err(|failure| Box::new(failure.problem()))?;
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|failure| Box::new(failure.problem()))?;
    super::project::rooted(ctx)?;
    let register = super::plugins::read(ctx)?;
    let project = super::project::current(ctx, &manifest, register.installed());
    let pass = if confirm {
        super::project::Pass::Reset
    } else {
        super::project::Pass::Preview
    };
    reverted.extend(
        super::project::materialised(ctx, &project.text, pass)
            .map_err(|failure| Box::new(failure.problem()))?,
    );
    if confirm {
        super::project::overlaid(ctx)?;
        super::project::secrets_written(ctx, &manifest, register.installed())
            .map_err(|failure| Box::new(failure.problem()))?;
    }

    // The connections a drifted service value reverts — the download-client categories the
    // operator changed. Read-only unless confirmed, so a preview lists them without writing.
    let reverted_connections = super::seed::reset_connections(ctx, confirm)
        .await
        .into_iter()
        .map(|wiring| wiring.connection)
        .collect();

    Ok((reverted, reverted_connections))
}

#[cfg(test)]
mod tests;
