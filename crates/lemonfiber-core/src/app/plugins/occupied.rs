//! What a plugin would take that something on this machine already holds.
//!
//! The manifest is held to the stack this build ships before anything here runs. What
//! that cannot see is this machine: the stack it runs, the overlay the operator merged
//! over it, the sites they added to the proxy by hand, and the ports other plugins
//! already publish. Each is a collision of the same kind, and each lands the same way —
//! a service Compose merges into the operator's own and runs with whatever that one
//! holds, a port two services cannot both bind, a site the proxy will not start beside a
//! second one at its address.
//!
//! **Refused, never renamed or narrowed**, for the reason the manifest's own refusals
//! are: a plugin installed under a name or a port its manifest does not carry is a
//! plugin nothing it declares is about any more. And refused with every clash at once,
//! so an operator does not find them one install at a time.

use std::path::{Path, PathBuf};

use lemonfiber_manifest::Manifest;

use crate::app::Ctx;
use crate::error::codes::plugin::OCCUPIED;
use crate::error::{Problem, Remedy, State};
use crate::model::MovedReport;
use crate::plugin::Installed;
use crate::project::overlay::{self, Declared, Published};

/// Refuse a plugin any of whose services would take a name, a port or a label that the
/// stack as this machine runs it, another installed plugin, or the live proxy already
/// holds.
///
/// `installed` is every other plugin on the machine, without this one's own record, so
/// an update is not held to the version it replaces.
///
/// # Errors
///
/// Where the stack's manifest cannot be read, or anything is taken, naming each clash
/// and what holds it.
pub(crate) fn unoccupied(
    ctx: &Ctx,
    would: &Installed,
    installed: &[Installed],
    stack: &Path,
) -> Result<(), Box<Problem>> {
    let settings = ctx
        .settings
        .env_file
        .as_deref()
        .and_then(|file| crate::config::store::read(file).ok())
        .unwrap_or_default();
    let manifest = super::writing::stack_manifest(ctx)?;
    let (mut services, mut ports) =
        run(&manifest, installed, &crate::app::beside::moved(&settings));
    let overlays: Vec<(PathBuf, String)> = ctx
        .settings
        .overlays
        .iter()
        .filter_map(|path| Some((path.clone(), std::fs::read_to_string(path).ok()?)))
        .collect();
    let (theirs, their_ports) =
        overlay::read(&overlays, &|name| settings.get(name).map(str::to_owned));
    services.extend(theirs);
    ports.extend(their_ports);
    let proxy = std::fs::read_to_string(stack.join(crate::plugin::PROXY)).unwrap_or_default();
    let mut clashes = named(would, &services);
    clashes.extend(bound(would, &ports, installed));
    clashes.extend(answered(would, &proxy));
    if clashes.is_empty() {
        return Ok(());
    }
    Err(Box::new(occupied(&would.plugin, &clashes)))
}

/// Every service the stack runs but one of `installed` stands in for, and every host port
/// its project publishes for it, where standing beside moved it to where `moved` says.
fn run(
    manifest: &Manifest,
    installed: &[Installed],
    moved: &[MovedReport],
) -> (Vec<Declared>, Vec<Published>) {
    let file = PathBuf::from(lemonfiber_manifest::assembly::ROOT);
    let mut services = Vec::new();
    let mut ports = Vec::new();
    for service in crate::project::written(manifest, installed) {
        services.push(Declared {
            service: service.id.clone(),
            file: file.clone(),
        });
        let published = service
            .port
            .into_iter()
            .chain(service.publish.iter().map(|further| further.port));
        for port in published {
            ports.push(Published {
                port: moved
                    .iter()
                    .find(|one| one.service == service.id && one.from == port)
                    .map_or(port, |one| one.to),
                service: service.id.clone(),
                file: file.clone(),
            });
        }
    }
    (services, ports)
}

/// Every service of the plugin's that the stack as this machine runs it already
/// declares by name.
fn named(would: &Installed, services: &[Declared]) -> Vec<String> {
    would
        .services
        .iter()
        .filter_map(|placed| {
            services
                .iter()
                .find(|held| held.service == placed.service)
                .map(|held| {
                    format!(
                        "service {}: {} already declares a service by that name, and Compose \
                         would merge the two into one that runs with everything that one holds",
                        placed.service,
                        held.file.display()
                    )
                })
        })
        .collect()
}

/// Every port the plugin would publish that the stack or another plugin already does.
fn bound(would: &Installed, ports: &[Published], installed: &[Installed]) -> Vec<String> {
    let mut clashes = Vec::new();
    for (placed, port) in would
        .services
        .iter()
        .flat_map(|placed| placed.ports().map(move |port| (placed, port)))
    {
        if let Some(held) = ports.iter().find(|held| held.port == port) {
            clashes.push(format!(
                "service {}: port {port} is already published for {} in {}",
                placed.service,
                held.service,
                held.file.display()
            ));
        }
        let theirs = installed
            .iter()
            .filter(|other| other.plugin != would.plugin)
            .find_map(|other| {
                other
                    .services
                    .iter()
                    .find(|one| one.ports().any(|held| held == port))
                    .map(|one| (other.plugin.as_str(), one.service.as_str()))
            });
        if let Some((plugin, service)) = theirs {
            clashes.push(format!(
                "service {}: port {port} is already published for {plugin}'s {service}",
                placed.service
            ));
        }
    }
    clashes
}

/// Every label the plugin would be proxied at that the live proxy configuration already
/// answers on, outside the plugin's own region.
fn answered(would: &Installed, proxy: &str) -> Vec<String> {
    let others = crate::region::without(proxy, &crate::plugin::owner(&would.plugin))
        .unwrap_or_else(|| proxy.to_owned());
    let held = lemonfiber_plugin::refusing::answered_in(&others);
    would
        .services
        .iter()
        .filter_map(|placed| {
            let label = placed.reached.as_ref()?.hostname()?;
            held.iter().any(|one| one == label).then(|| {
                format!(
                    "service {}: the proxy's configuration already has a site at {label}, and \
                     it will not start with two",
                    placed.service
                )
            })
        })
        .collect()
}

/// Something on this machine already holds what the plugin would take.
fn occupied(plugin: &str, clashes: &[String]) -> Problem {
    Problem::new(
        OCCUPIED,
        format!("{plugin} would take what this machine already uses"),
        "Nothing was written. Each service lemonfiber writes for a plugin is a container by \
         its id, published on its port and proxied at its label, and each of those is already \
         held here.",
        Remedy::new(format!(
            "Rename or move what holds them, or install a version of {plugin} that declares \
             others"
        )),
    )
    .in_state(State::Guided)
    .with_detail(clashes.join("; "))
}

#[cfg(test)]
mod tests;
