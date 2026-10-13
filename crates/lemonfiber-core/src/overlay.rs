//! The operator's own overlay: one Compose file, `overlay.yml` in lemonfiber's
//! configuration directory, merged last over the project lemonfiber writes.
//!
//! Nothing in it was held to the model, so wherever it changes something it is named as
//! unvetted with every service it changes, and each change to it is journalled by content
//! when lemonfiber next starts the project. Its place is fixed: no setting names a second
//! file or moves this one, and a path still recorded under the retired
//! `LEMONFIBER_OVERLAY` key is not read.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::config::env::EnvFile;
use crate::config::store::Failure;
use crate::journal::{Change, Kind};
use crate::model::Unvetted;

/// The overlay's name in lemonfiber's configuration directory.
pub const FILE: &str = "overlay.yml";

/// The operation a change to the overlay is journalled under.
pub(crate) const OPERATION: &str = "overlay";

/// Where the overlay is kept beside `env_file`, lemonfiber's own settings.
#[must_use]
pub fn at(env_file: &Path) -> PathBuf {
    env_file.with_file_name(FILE)
}

/// The overlay, where the operator has written one.
#[must_use]
pub fn written(env_file: Option<&Path>) -> Option<PathBuf> {
    env_file.map(at).filter(|path| path.is_file())
}

/// Every Compose file layered over the stack, in the order Compose reads them: the moves
/// standing beside an existing setup recorded, then the operator's overlay, last.
#[must_use]
pub fn layered(env_file: Option<&Path>, held: &EnvFile) -> Vec<PathBuf> {
    env_file
        .and_then(|env| crate::app::beside::layer(env, held))
        .into_iter()
        .chain(written(env_file))
        .collect()
}

/// The overlay and every service it changes, where there is one.
#[must_use]
pub fn unvetted(ctx: &Ctx) -> Option<Unvetted> {
    let path = written(ctx.settings.env_file.as_deref())?;
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    Some(Unvetted {
        overlay: path.display().to_string(),
        services: crate::stack::declared::named(&text),
    })
}

/// Journal the overlay where it differs by content from what the journal last recorded
/// of it: written, changed or taken away.
///
/// # Errors
///
/// Where the journal cannot be read or written.
pub(crate) fn journalled(ctx: &Ctx) -> Result<(), Failure> {
    if ctx.dry_run {
        return Ok(());
    }
    let Some(env) = ctx.settings.env_file.as_deref() else {
        return Ok(());
    };
    let Some(paths) = crate::app::targets::layout(ctx) else {
        return Ok(());
    };
    let path = at(env);
    let current = std::fs::read_to_string(&path)
        .ok()
        .map(|text| crate::credential::fingerprint(&text));
    let journal = paths.journal();
    let previous = crate::app::recover::journal_at(&journal)?
        .changes()
        .iter()
        .rev()
        .find_map(|change| match &change.kind {
            Kind::Overlaid { current, .. } => Some(current.clone()),
            _ => None,
        })
        .flatten();
    if previous == current {
        return Ok(());
    }
    let change = Change {
        at: ctx.stamp(),
        operation: OPERATION.to_owned(),
        target: path.display().to_string(),
        kind: Kind::Overlaid {
            path: path.display().to_string(),
            previous,
            current,
        },
    };
    crate::app::recover::journalled(&journal, &[change], ctx.seams.random.as_ref())
}

#[cfg(test)]
mod tests;
