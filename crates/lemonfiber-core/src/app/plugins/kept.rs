//! The starting files an installed plugin's run proofs are run with again, kept beside
//! the install record once the install holds and taken away with the plugin.

use std::path::{Path, PathBuf};

use crate::config::paths::STARTING_KEPT;
use crate::plugin::program::{keep, Starting};
use crate::plugin::Installed;

use super::super::Ctx;

/// Where the starting files kept for `plugin` are, where anywhere is configured.
pub(crate) fn root(ctx: &Ctx, plugin: &str) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, STARTING_KEPT).map(|kept| kept.join(plugin))
}

/// Keep a copy of the starting files of every service `installed` runs a proof of, from
/// the plugin's source at `source`, in place of any kept before.
pub(crate) fn kept(ctx: &Ctx, installed: &Installed, source: &Path) {
    let Some(into) = root(ctx, &installed.plugin) else {
        return;
    };
    let _ = std::fs::remove_dir_all(&into);
    for service in proved(installed) {
        let _ = keep(
            Starting {
                root: source,
                service: &service,
            },
            &into,
        );
    }
}

/// Take away every starting file kept for `plugin`.
pub(crate) fn forgotten(ctx: &Ctx, plugin: &str) {
    if let Some(at) = root(ctx, plugin) {
        let _ = std::fs::remove_dir_all(at);
    }
}

/// Every service `installed` runs a proof of, once each.
fn proved(installed: &Installed) -> Vec<String> {
    let mut services: Vec<String> = installed
        .run_proofs
        .iter()
        .map(|one| one.service.clone())
        .collect();
    services.sort();
    services.dedup();
    services
}

#[cfg(test)]
mod tests;
