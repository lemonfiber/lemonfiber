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

/// Take away every starting file kept for `plugin`.
pub(crate) fn forgotten(ctx: &Ctx, plugin: &str) {
    if let Some(at) = root(ctx, plugin) {
        let _ = std::fs::remove_dir_all(at);
    }
}

/// Every service `installed` runs a proof of or whose starting files at `source` name a
/// secret, once each.
fn proved(installed: &Installed, source: &Path) -> Vec<String> {
    let seeded = crate::plugin::seeding::shipped(
        installed
            .services
            .iter()
            .map(|placed| placed.service.as_str()),
        source,
    )
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
