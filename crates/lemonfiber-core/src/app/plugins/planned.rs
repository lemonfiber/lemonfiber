//! What installing a plugin writes: its own directories, keys, secrets and starting files,
//! and the project generated with it beside everything else installed here.

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
/// Where `DATA_ROOT` is refused, the stack's manifest or the record of what is installed
/// cannot be read, or the project was edited since lemonfiber last wrote it.
pub(crate) fn writes(
    ctx: &Ctx,
    installed: &Installed,
    stack: &Path,
    source: Option<&Path>,
) -> Result<Vec<Write>, Box<Problem>> {
    let starting = match source {
        Some(source) => crate::plugin::seeding::shipped_by(installed, source)
            .map_err(|why| Box::new(unshipped(&installed.plugin, &why)))?,
        None => Vec::new(),
    };
    let starting = crate::plugin::seeding::starting(starting, stack, &|at| at.exists());
    super::super::project::rooted(ctx)?;
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
    let at = stack.join(crate::project::FILE);
    let previous = super::super::project::held(ctx);
    let project = (previous.as_deref() != Some(project.text.as_str()))
        .then(|| Write::project(at, previous, project.text));
    Ok(resolved
        .reading(|reading| crate::plugin::writes(installed, stack, reading, starting, project)))
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

/// Why a plugin whose starting files were read when it was accepted could not have them
/// read again to be written.
fn unshipped(plugin: &str, why: &str) -> Problem {
    Problem::new(
        crate::error::codes::plugin::UNREADABLE,
        format!("{plugin}'s starting files could not be read"),
        "Nothing was installed and nothing was written.",
        crate::error::Remedy::new("Check the plugin's config directory, then install it again"),
    )
    .with_detail(why.to_owned())
}
