//! Asking the stack's own checks what they make of the machine, twice.
//!
//! **What a plugin's own proofs cannot see is the whole reason this runs.** A proof
//! asks the plugin's service whether it does what the plugin said it does. It has no
//! way to notice that the container took a port another service was listening on, or
//! that the disk the library sits on no longer has room, or that the tunnel stopped
//! carrying what it was carrying — and those are the things an operator finds out
//! about days later. The stack's own diagnosis notices all three and is already
//! written.
//!
//! **Read twice and compared, never read once and judged.** A machine with a failing
//! indexer key is a machine that fails a diagnosis, and an install refused on those
//! grounds would be refused for something it did not do and cannot fix. So the first
//! reading is taken before anything is written and is the thing the second is held
//! against; [`crate::plugin::against`] is the rule, and lives with the vocabulary
//! because every case of it can be decided without a machine.
//!
//! **Neither reading is disruptive.** Proving an install is no reason to drop the
//! default route or spend a live indexer search, twice — which is the same care a
//! repair takes when it proves its own work, for the same reason.

use lemonfiber_manifest::Manifest;

use crate::doctor::{Finding, Narrowing};
use crate::error::Problem;

use super::super::Ctx;

/// The first reading, and the one thing in this that can refuse.
///
/// Answers with the manifest as well as the findings, because the second reading is
/// built from the same document: what can be read has been read by then, and a second
/// read that could fail where the first did not would be a refusal arriving after the
/// machine had already been written to.
///
/// # Errors
///
/// Where the stack's own manifest cannot be read, which is the one thing every check
/// needs before any of them can run. Its one caller asks it before a byte of the
/// install is written, so a refusal here leaves nothing to put back.
pub(super) async fn looked(ctx: &Ctx) -> Result<(Manifest, Vec<Finding>), Box<Problem>> {
    let (manifest, checks) = super::super::engine::assembled(ctx, false).await?;
    let findings = examined(ctx, &manifest, &checks).await;
    Ok((manifest, findings))
}

/// The second reading, over checks built afresh from the manifest the first read.
///
/// Fresh instances rather than the first reading's, and that is the point of asking
/// again: a check holds what it read when it was built, so putting the same instances
/// a second question would compare the install against the very reading it was meant
/// to change — and would report every install as having broken nothing.
pub(super) async fn again(ctx: &Ctx, manifest: &Manifest) -> Vec<Finding> {
    let checks = super::super::engine::assembling(ctx, manifest, false).await;
    examined(ctx, manifest, &checks).await
}

/// One reading, through the pairing every other caller's goes through.
///
/// Through [`crate::app::engine::examined`] rather than the bare runner, so a warning
/// the operator has already answered for is marked as answered on both sides. A
/// reading that skipped that would have a settled question read as freshly wrong on
/// one side of the comparison and not the other, which is a difference the install
/// would be blamed for.
async fn examined(
    ctx: &Ctx,
    manifest: &Manifest,
    checks: &[Box<dyn crate::doctor::Check>],
) -> Vec<Finding> {
    super::super::engine::examined(ctx, &manifest.services, checks, &Narrowing::Suite)
        .await
        .findings
}
