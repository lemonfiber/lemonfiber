//! What installing a plugin writes: its own directories, keys and secrets files, its
//! regions in the stack's files, and the project generated with it beside everything
//! else installed here.

use std::path::Path;

use super::super::Ctx;
use crate::error::Problem;
use crate::plugin::{Installed, Write};
use crate::settings::Resolved;

/// What installing `installed` writes beside everything else installed here, in place of
/// any version of it already among them.
///
/// # Errors
///
/// Where the stack's manifest or the record of what is installed cannot be read, or the
/// project was edited since lemonfiber last wrote it.
pub(crate) fn writes(
    ctx: &Ctx,
    installed: &Installed,
    stack: &Path,
) -> Result<Vec<Write>, Box<Problem>> {
    super::super::project::unedited(ctx)?;
    let manifest = super::writing::stack_manifest(ctx)?;
    let beside = beside(&super::read(ctx)?, installed);
    let resolved = Resolved::of(ctx, Some(&manifest), &beside);
    let project = super::super::project::generated(
        &manifest,
        &beside,
        &crate::app::targets::chosen_fillers(ctx),
        &resolved,
    );
    let at = super::super::project::path(ctx).unwrap_or_else(|| stack.join(crate::project::FILE));
    let previous = super::super::project::held(ctx);
    let project = (previous.as_deref() != Some(project.text.as_str()))
        .then(|| Write::project(at, previous, project.text));
    Ok(resolved.reading(|reading| crate::plugin::writes(installed, stack, reading, project)))
}

/// What `register` holds with `installed` in place of any version of it.
pub(crate) fn beside(register: &crate::plugin::Register, installed: &Installed) -> Vec<Installed> {
    register
        .installed()
        .iter()
        .filter(|one| one.plugin != installed.plugin)
        .cloned()
        .chain(std::iter::once(installed.clone()))
        .collect()
}
