//! The parts the bundle installs that this stack still runs as its own service: each a
//! move in place on offer (ARCH-R299).

use lemonfiber_bundled::{Pinned, Stage, PINNED};
use lemonfiber_manifest::{Manifest, Service};
use schemars::JsonSchema;
use serde::Serialize;

use crate::app::Ctx;
use crate::plugin::Installed;

/// A move in place the bundle offers: the plugin and the stack's own service of its id it
/// would take the place of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct MoveOffered {
    /// The plugin the bundle installs.
    pub plugin: String,
    /// The stack's own service it would take the place of, under the same container name,
    /// configuration directory, data and names.
    pub service: String,
}

/// Every part `pinned` installs whose plugin is not among `installed` while `stack` still
/// runs a service of its id.
#[must_use]
pub fn offered(pinned: &[Pinned], stack: &[Service], installed: &[Installed]) -> Vec<MoveOffered> {
    pinned
        .iter()
        .filter(|one| one.stage == Some(Stage::Installed))
        .map(|one| one.first_party.plugin)
        .filter(|plugin| !installed.iter().any(|held| held.plugin == *plugin))
        .filter(|plugin| stack.iter().any(|service| service.id == *plugin))
        .map(|plugin| MoveOffered {
            plugin: plugin.to_owned(),
            service: plugin.to_owned(),
        })
        .collect()
}

/// The moves on offer on this machine, against the stack `manifest` declares: nothing
/// where its record of what is installed cannot be read, since a move offered over a
/// plugin already installed would be one nothing can take.
#[must_use]
pub(crate) fn on_offer(ctx: &Ctx, manifest: &Manifest) -> Vec<MoveOffered> {
    super::read(ctx)
        .map(|register| offered(PINNED, &manifest.services, register.installed()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
