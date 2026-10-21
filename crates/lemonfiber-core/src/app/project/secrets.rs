//! Each service's own secrets file: every variable its environment names a secret for,
//! with the value read from where it is held, in a file no container mounts.

use lemonfiber_manifest::Manifest;

use super::super::Ctx;
use crate::config::store::{self, Failure};
use crate::plugin::Installed;
use crate::settings::Resolved;

/// Write the secrets file of every service of the stack and of `installed` whose
/// environment names a secret.
///
/// # Errors
///
/// Where a store cannot be read or a file cannot be written.
pub(crate) fn written(
    ctx: &Ctx,
    manifest: &Manifest,
    installed: &[Installed],
) -> Result<(), Failure> {
    let resolved = Resolved::of(ctx, Some(manifest), installed);
    for (_, path, variables) in
        crate::settings::files::every_file(Some(manifest), installed, &resolved)
    {
        store::write(&path, &crate::settings::text(ctx, &variables)?)?;
    }
    Ok(())
}
