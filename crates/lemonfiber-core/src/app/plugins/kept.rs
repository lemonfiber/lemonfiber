//! What an installed plugin is run with again once its source may be gone: the starting
//! files its run proofs use and the manifest its demand recipes are read from, kept
//! beside the install record once the install holds and taken away with the plugin.

use std::path::{Path, PathBuf};

use lemonfiber_bundle::MANIFEST;

use crate::config::paths::STARTING_KEPT;
use crate::plugin::program::{keep, Starting};
use crate::plugin::Installed;

use super::super::Ctx;

/// Where the starting files kept for `plugin` are, where anywhere is configured.
pub(crate) fn root(ctx: &Ctx, plugin: &str) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, STARTING_KEPT).map(|kept| kept.join(plugin))
}

/// Keep a copy of the plugin's manifest and of the starting files of every service
/// `installed` runs a proof of or whose files name a secret, from the plugin's source at
/// `source`, in place of any kept before.
pub(crate) fn kept(ctx: &Ctx, installed: &Installed, source: &Path) {
    let Some(into) = root(ctx, &installed.plugin) else {
        return;
    };
    let _ = std::fs::remove_dir_all(&into);
    if std::fs::create_dir_all(&into).is_ok() {
        let _ = std::fs::copy(source.join(MANIFEST), into.join(MANIFEST));
    }
    for service in proved(installed, source) {
        let _ = keep(
            Starting {
                root: source,
                service: &service,
            },
            &into,
        );
    }
}

/// The manifest kept for `installed`, read and held to the digest its record holds.
///
/// # Errors
///
/// Where nowhere is configured to keep it, none was kept, it cannot be read, or what is
/// kept is not the manifest that was installed; said as why, for the caller's problem.
pub(crate) fn manifest(
    ctx: &Ctx,
    installed: &Installed,
) -> Result<lemonfiber_plugin::Manifest, String> {
    let at = root(ctx, &installed.plugin)
        .ok_or_else(|| "there is nowhere configured that its manifest is kept".to_owned())?;
    let read = crate::plugin::read_digested(&at)
        .map_err(|unread| format!("the manifest kept when it was installed: {unread}"))?;
    if read.digest != installed.manifest {
        return Err(
            "the manifest kept when it was installed is not the one that was installed".to_owned(),
        );
    }
    Ok(read.manifest)
}

/// Each starting file whose placeholders `would`, from its source at `path`, ships
/// differently from what was kept for `was`, beneath `stack`. Read off what each version
/// ships alone, never off the files on the machine, which hold the values.
pub(crate) fn reshipped(
    ctx: &Ctx,
    was: &Installed,
    would: &Installed,
    stack: &Path,
    path: &Path,
) -> Vec<crate::plugin::Reshipping> {
    let shipped = |installed: &Installed, source: &Path| {
        crate::plugin::seeding::shipped_by(installed, source).unwrap_or_default()
    };
    let before = root(ctx, &was.plugin)
        .map(|kept| shipped(was, &kept))
        .unwrap_or_default();
    let after = shipped(would, &crate::plugin::root(path));
    crate::plugin::reshipped(stack, &before, &after)
}

/// Whether `change` made a starting file of one of `was`'s services beneath `stack`, which
/// an update leaves in place rather than putting back: the service has rewritten it as it
/// ran by then, and the new version writes a starting file only where none is yet.
///
/// A file beneath a service's configuration directory other than the key it is asked with
/// and the credentials an adapter reads its upstream's with, both of which every install
/// writes afresh.
pub(crate) fn in_place(was: &Installed, stack: &Path, change: &crate::journal::Change) -> bool {
    let crate::journal::Kind::Made { path } = &change.kind else {
        return false;
    };
    let at = Path::new(path);
    at.is_file()
        && was.services.iter().any(|placed| {
            let directory = crate::plugin::configuration(stack, &placed.service);
            at.starts_with(&directory)
                && at != crate::plugin::key_file(stack, &placed.service)
                && at != directory.join(lemonfiber_contract::adapter::UPSTREAM_FILE)
        })
}

/// Take away every starting file kept for `plugin`.
pub(crate) fn forgotten(ctx: &Ctx, plugin: &str) {
    if let Some(at) = root(ctx, plugin) {
        let _ = std::fs::remove_dir_all(at);
    }
}

/// Every service `installed` runs a proof of or whose starting files at `source` name a
/// secret, once each.
fn proved(installed: &Installed, source: &Path) -> Vec<String> {
    let seeded = crate::plugin::seeding::shipped_by(installed, source)
        .unwrap_or_default()
        .into_iter()
        .filter(|file| crate::plugin::seeding::seeded(&file.bytes))
        .map(|file| file.service);
    let mut services: Vec<String> = installed
        .run_proofs
        .iter()
        .map(|one| one.service.clone())
        .chain(seeded)
        .collect();
    services.sort();
    services.dedup();
    services
}

#[cfg(test)]
mod tests;
