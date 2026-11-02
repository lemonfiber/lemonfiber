//! The parts the bundle installs that this stack still runs as its own service: each a
//! move in place on offer, where the service has run on this machine.

use std::path::PathBuf;

use async_trait::async_trait;
use lemonfiber_bundled::{Pinned, Stage, PINNED};
use lemonfiber_manifest::{Manifest, Service};
use schemars::JsonSchema;
use serde::Serialize;

use crate::app::Ctx;
use crate::doctor::moving::Offers;
use crate::error::Problem;
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
    /// The part's adapter that writes the configuration the service reads, where one does:
    /// it runs only once the move is taken, so until then the service starts without what
    /// that configuration holds, the credentials it reaches other services with included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured_by: Option<String>,
}

/// Every part `pinned` installs whose plugin is not among `installed` while `stack` still
/// runs a service of its id that `ran` says has run on this machine.
#[must_use]
pub fn offered(
    pinned: &[Pinned],
    stack: &[Service],
    installed: &[Installed],
    ran: &Ran,
) -> Vec<MoveOffered> {
    uninstalled(pinned, installed)
        .into_iter()
        .filter(|one| {
            stack
                .iter()
                .any(|service| service.id == one.first_party.plugin)
        })
        .filter(|one| ran.has_run(one.first_party.plugin))
        .map(|one| MoveOffered {
            plugin: one.first_party.plugin.to_owned(),
            service: one.first_party.plugin.to_owned(),
            configured_by: configuring(one.copy, one.first_party.plugin),
        })
        .collect()
}

/// Every part `pinned` installs whose plugin is not among `installed`.
pub(crate) fn uninstalled<'a>(pinned: &'a [Pinned], installed: &[Installed]) -> Vec<&'a Pinned> {
    pinned
        .iter()
        .filter(|one| one.stage == Some(Stage::Installed))
        .filter(|one| {
            !installed
                .iter()
                .any(|held| held.plugin == one.first_party.plugin)
        })
        .collect()
}

/// What says whether the stack's own service of an id has run on this machine: the
/// configuration directories beneath the stack, and every container the engine holds
/// for the project.
#[derive(Debug, Clone, Default)]
pub struct Ran {
    /// Where the stack is written, where it is.
    stack: Option<PathBuf>,
    /// The service of every container the engine holds for the project.
    containers: Vec<String>,
}

impl Ran {
    /// What `stack` and the engine's `containers` say has run.
    #[must_use]
    pub fn of(stack: Option<PathBuf>, containers: Vec<String>) -> Self {
        Self { stack, containers }
    }

    /// What this machine says has run: nothing the engine holds where it cannot be asked.
    pub(crate) async fn here(ctx: &Ctx) -> Self {
        let containers = ctx
            .seams
            .engine
            .list(&ctx.settings.project)
            .await
            .map(|held| held.into_iter().map(|one| one.service).collect())
            .unwrap_or_default();
        Self::of(crate::app::project::directory(ctx), containers)
    }

    /// Whether the stack's own service `service` has run here: its configuration
    /// directory beneath the stack holds anything, or the engine holds a container for it.
    #[must_use]
    pub fn has_run(&self, service: &str) -> bool {
        self.containers.iter().any(|held| held == service)
            || self.stack.as_deref().is_some_and(|stack| {
                std::fs::read_dir(crate::plugin::configuration(stack, service))
                    .is_ok_and(|mut entries| entries.next().is_some())
            })
    }
}

/// The service of the manifest `copy` that fronts `service` and shares its configuration
/// directory, where one does.
fn configuring(copy: &str, service: &str) -> Option<String> {
    lemonfiber_plugin::Manifest::from_toml(copy)
        .ok()?
        .services
        .into_iter()
        .find(|one| one.fronts.as_deref() == Some(service) && one.shares_config.is_some())
        .map(|one| one.id)
}

/// The moves on offer on this machine, against the stack `manifest` declares: nothing
/// where its record of what is installed cannot be read, since a move offered over a
/// plugin already installed would be one nothing can take.
///
/// lemonfiber's own guards are not among them: they move with the next act that keeps
/// the stack running ([`super::guarding`]), never with an update's yes.
pub(crate) async fn on_offer(ctx: &Ctx, manifest: &Manifest) -> Vec<MoveOffered> {
    all_on_offer(ctx, manifest)
        .await
        .into_iter()
        .filter(|offered| !super::guarding::guards(&offered.plugin))
        .collect()
}

/// Every move on offer on this machine against `manifest`, the guards' included.
pub(crate) async fn all_on_offer(ctx: &Ctx, manifest: &Manifest) -> Vec<MoveOffered> {
    here(ctx, &manifest.services).await.unwrap_or_default()
}

/// The moves on offer on this machine against `services`, or why its record of what is
/// installed cannot be read.
async fn here(ctx: &Ctx, services: &[Service]) -> Result<Vec<MoveOffered>, Box<Problem>> {
    let register = super::read(ctx)?;
    let ran = Ran::here(ctx).await;
    Ok(offered(PINNED, services, register.installed(), &ran))
}

/// This machine's moves on offer against a stack's services, read each time it is asked.
pub(crate) struct Offering {
    /// Where the record of what is installed is read.
    ctx: Ctx,
    /// The services the stack declares.
    services: Vec<Service>,
}

impl Offering {
    /// The moves on offer against what `manifest` declares, read through `ctx`.
    #[must_use]
    pub(crate) fn of(ctx: &Ctx, manifest: &Manifest) -> Self {
        Self {
            ctx: ctx.clone(),
            services: manifest.services.clone(),
        }
    }
}

#[async_trait]
impl Offers for Offering {
    async fn moves(&self) -> Result<Vec<MoveOffered>, ()> {
        here(&self.ctx, &self.services).await.map_err(|_| ())
    }
}

#[cfg(test)]
mod tests;
