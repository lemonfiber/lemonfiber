//! What this machine keeps of lemonfiber's, and taking it off.
//!
//! Two errands over one answer. Listing touches nothing; forgetting removes the two
//! directories everything sits under and says what went. Both answer with the same
//! shape, because what is agreed to has to be what was read — a removal that
//! summarised what a listing had said in different words would be a second account
//! of the same thing, and the operator would be agreeing to the summary.
//!
//! Nothing happens without the agreement, and a rehearsal is not one. `--dry-run`
//! reaches here as a run that changes nothing, which is what it means everywhere
//! else; it reports what a confirmed run would take rather than taking it.

use crate::config::paths::Paths;
use crate::error::{Amiss, Code, Problem, Remedy, Severity};
use crate::stored::{stored, Left, Removal, Stored};

use super::Ctx;

/// Raised when this run does not know where lemonfiber's own files go.
pub const NOWHERE_KNOWN: Code = Code::new("KEPT-1");

/// What lemonfiber keeps on this machine, listed and not touched.
///
/// # Errors
///
/// Returns a [`Problem`] where this run could not resolve where its own files go,
/// which is the one thing a listing cannot answer around.
pub(super) fn listing(ctx: &Ctx) -> Result<Stored, Box<Problem>> {
    Ok(stored(layout(ctx)?, Removal::NotAsked))
}

/// Remove everything lemonfiber keeps on this machine, or say what that would be.
///
/// # Errors
///
/// Returns a [`Problem`] where this run could not resolve where its own files go.
pub(super) async fn forgetting(ctx: &Ctx, confirm: bool) -> Result<Stored, Box<Problem>> {
    let paths = layout(ctx)?.clone();
    if !confirm || ctx.dry_run {
        return Ok(stored(&paths, Removal::Unconfirmed));
    }
    let mut gone = Vec::new();
    let mut left = Vec::new();
    for root in [paths.config_dir(), paths.data_dir()] {
        match ctx.eraser.erase(root).await {
            Ok(()) => gone.push(root.display().to_string()),
            Err(fault) => left.push(Left {
                at: root.display().to_string(),
                why: fault.message,
            }),
        }
    }
    Ok(stored(&paths, Removal::Done { gone, left }))
}

/// Where this machine keeps lemonfiber's files.
fn layout(ctx: &Ctx) -> Result<&Paths, Box<Problem>> {
    ctx.archives
        .as_ref()
        .map(|archiving| &archiving.paths)
        .ok_or_else(|| Box::new(nowhere()))
}

/// The refusal a run that cannot say where its own files go is answered with.
///
/// Not a guess at the usual place. A machine whose configuration home could not be
/// resolved is one whose files are somewhere this run cannot name, and answering
/// with the ordinary path would tell somebody to go and delete a directory that may
/// hold somebody else's things.
fn nowhere() -> Problem {
    Problem::new(
        NOWHERE_KNOWN,
        Severity::Error,
        "this run cannot say where lemonfiber keeps its own files",
        "The configuration and data directories are resolved from this machine's own \
         conventions, and that did not work here — so there is nothing to list and nothing \
         safe to remove. Guessing at the usual place would risk naming a directory that is \
         somebody else's.",
        Remedy::new(
            "Run this as the account that installed lemonfiber, on a machine with a home \
             directory it can read",
        ),
    )
    .lies_in(Amiss::Answering)
}
