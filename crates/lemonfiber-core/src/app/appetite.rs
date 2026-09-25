//! What the operator asked to hear about, kept between runs.
//!
//! Read wherever a digest is built and written whenever the answer changes, so the
//! preset chosen once at setup is the one still in force a month later — and so
//! the individual events switched on or off since are too.
//!
//! Read best-effort and written strictly, the same way the quality choice is. A
//! run that cannot read the file falls back to the quiet default, which is the
//! safe direction: an operator hears less than they asked for rather than being
//! refused a command. A run that cannot *write* it says so, because that is the
//! operator's explicit action and silently losing it would leave them believing
//! they had changed something they had not.

use std::path::PathBuf;

use crate::alert::Wants;
use crate::config::store;
use crate::error::{Diagnose, Problem};
use crate::model::{AlertReport, ExceptionReport};

use super::{AlertAction, Ctx};

/// What the operator asked to hear about, or the quiet default where they have not
/// said and where the answer cannot be read.
#[must_use]
pub fn recorded(ctx: &Ctx) -> Wants {
    super::record::kept(path(ctx).as_deref())
}

/// Record the answer where the next run — and a backup — will find it.
///
/// Reported rather than swallowed on failure: this is the operator's explicit
/// action, and an answer that quietly did not persist is worse than one that
/// visibly could not.
///
/// # Errors
///
/// Where there is nowhere configured to keep it, or the file cannot be written.
pub fn record(ctx: &Ctx, wants: &Wants) -> Result<(), Box<Problem>> {
    let path = path(ctx).ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    store::write(&path, &serde_json::to_string(wants).unwrap_or_default())
        .map_err(|failure| Box::new(failure.problem()))
}

/// Show what the operator will be told about, or change it.
///
/// The preset is an answer setup asks for once. Without this it could not be revised at
/// all — the file is written when setup applies and read whenever a digest is built, and
/// nothing in between could change it. A decision made once and then unchangeable is a
/// trap, whatever its default.
///
/// Taking a preset leaves the individual exceptions in place: a broader answer is not a
/// reason to discard the specific ones already given.
///
/// # Errors
///
/// Where there is nowhere configured to keep the answer, or it cannot be written.
pub fn alerts(ctx: &Ctx, action: AlertAction) -> Result<AlertReport, Box<Problem>> {
    let mut wants = recorded(ctx);
    let changed = match action {
        AlertAction::Show => false,
        AlertAction::Set(preset) => {
            wants.choose(preset);
            // A rehearsal reports the answer it would have kept without keeping it,
            // which is what `--dry-run` means everywhere.
            if !ctx.dry_run {
                record(ctx, &wants)?;
            }
            !ctx.dry_run
        }
    };
    let preset = Wants::appetite(&wants);
    Ok(AlertReport {
        preset: preset.written().to_owned(),
        means: preset.describe().to_owned(),
        exceptions: wants
            .exceptions()
            .map(|(kind, wanted)| ExceptionReport {
                kind: kind.to_owned(),
                wanted,
            })
            .collect(),
        changed,
        rehearsed: ctx.dry_run,
    })
}

/// Where the answer is kept: beside the environment file, in the configuration
/// directory a backup captures, or nowhere when nothing is configured. Equal to
/// [`crate::config::paths::Paths::notifications`].
fn path(ctx: &Ctx) -> Option<PathBuf> {
    super::targets::beside_env(ctx, "notifications.json")
}

#[cfg(test)]
mod tests;
