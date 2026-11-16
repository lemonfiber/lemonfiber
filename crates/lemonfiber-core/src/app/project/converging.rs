//! The containers a start creates again, and every other container of the project brought
//! onto the networks its service joins where it runs.
//!
//! The program running the project creates a container again whenever its service's
//! networks change, so a start tells it to create only what is missing and decides itself
//! what goes first. Each service's digest is read from the project as written: its entry
//! apart from what orders and selects services and the networks it joins, each overlay's
//! entry for it the same, and every secrets file it names. Each is written into the
//! project's labels file, so a container created now is labelled with it. A container the
//! start reaches goes where its label differs, the image its service names is another one
//! here than the one it runs, or it was on a network created again, so the start creates it
//! again. Every other container is connected to each network its service joins and
//! disconnected from each network of the project it no longer joins, and one the engine
//! would not connect or disconnect goes as well where the start reaches it. A service's
//! digest takes in that of each service it runs inside the network of, and a container
//! running inside the network of one that goes goes too, so none is left on a namespace that
//! went. Where a container cannot be taken away, the start is refused.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use lemonfiber_ports::docker::{Container, Endpoint, Joining, ENTRY_LABEL};

use super::super::Ctx;
use crate::error::codes::stack::CONTAINER_NOT_RENEWED;
use crate::error::{Problem, Remedy};
use crate::ports::digest::sha256;

/// What the project as written wants of one service.
#[derive(Debug, Default)]
struct Wanted {
    /// The digest of what its container is created from.
    digest: String,
    /// Each network it joins.
    networks: Vec<crate::project::document::Joined>,
    /// Each service it waits for.
    depends: Vec<String>,
    /// The image it runs, as the project names it.
    image: Option<String>,
    /// The service whose network it runs inside.
    inside: Option<String>,
}

/// Bring every container of the project onto its service's networks, take away first each
/// one a start of `scope`, or of everything where `scope` is empty, creates again, by
/// running the command `removing` builds for those services, and label what it creates.
///
/// # Errors
///
/// Where the containers to be created again could not be taken away.
pub(crate) async fn renewed(
    ctx: &Ctx,
    scope: &[String],
    displaced: &[String],
    removing: impl FnOnce(Vec<String>) -> Vec<String>,
) -> Result<(), Box<Problem>> {
    let gone = converged(ctx, scope, displaced).await;
    if gone.is_empty() {
        return Ok(());
    }
    let command = removing(gone.clone());
    match ctx.seams.runner.run(&command).await {
        Ok(output) if output.succeeded() => Ok(()),
        Ok(refused) => Err(unrenewed(&gone, refused.stderr.trim())),
        Err(why) => Err(unrenewed(&gone, &why.to_string())),
    }
}

/// The refusal of a start whose containers `gone` could not be taken away first.
fn unrenewed(gone: &[String], why: &str) -> Box<Problem> {
    Box::new(
        Problem::new(
            CONTAINER_NOT_RENEWED,
            format!(
                "The containers of {} could not be taken away to be created again, so nothing \
                 was started",
                gone.join(", ")
            ),
            "lemonfiber creates these containers again from the project as written, and one \
             started now would run as it was created, outside its fence or on an image the \
             project no longer names.",
            Remedy::new("Check the container engine is running, then start again"),
        )
        .with_detail(why.to_owned()),
    )
}

/// Each service of `scope`, or of everything, whose container a start creates again, every
/// other container of the project brought onto its service's networks.
async fn converged(ctx: &Ctx, scope: &[String], displaced: &[String]) -> Vec<String> {
    let Some((text, wanted, unlabelled)) = written(ctx) else {
        return Vec::new();
    };
    if let Some(why) = unlabelled {
        ctx.narrator
            .say(&format!(
                "the labels of the project could not be written: {why}"
            ))
            .await;
    }
    let reached = reached(&wanted, scope);
    let owned = crate::project::document::networks_named(&text, &ctx.settings.project);
    let containers = ctx
        .seams
        .engine
        .list(&ctx.settings.project)
        .await
        .unwrap_or_default();
    let mut renewed = BTreeSet::new();
    for container in &containers {
        let Some(one) = wanted.get(&container.service) else {
            continue;
        };
        let within = reached.contains(container.service.as_str());
        let stale = container.entry.as_deref() != Some(one.digest.as_str())
            || displaced.contains(&container.service)
            || (within && moved(ctx, container, one).await);
        let kept = !(within && stale) && joined(ctx, container, one, &owned).await;
        if within && !kept {
            renewed.insert(container.service.clone());
        }
    }
    with_insiders(&wanted, &containers, renewed)
        .into_iter()
        .collect()
}

/// `renewed`, and every service with a container in `containers` running inside the network
/// of one of them, through every service each runs inside: a container whose namespace is
/// created again is left on the one that went.
fn with_insiders(
    wanted: &BTreeMap<String, Wanted>,
    containers: &[Container],
    mut renewed: BTreeSet<String>,
) -> BTreeSet<String> {
    loop {
        let more: Vec<String> = containers
            .iter()
            .filter(|container| !renewed.contains(&container.service))
            .filter(|container| {
                wanted
                    .get(&container.service)
                    .and_then(|one| one.inside.as_ref())
                    .is_some_and(|host| renewed.contains(host))
            })
            .map(|container| container.service.clone())
            .collect();
        if more.is_empty() {
            return renewed;
        }
        renewed.extend(more);
    }
}

/// The project as written beneath the project directory, what each service it declares
/// wants, and why its labels could not be written, where they could not; nothing where no
/// project is written.
fn written(ctx: &Ctx) -> Option<(String, BTreeMap<String, Wanted>, Option<String>)> {
    let directory = super::directory(ctx)?;
    let text = std::fs::read_to_string(directory.join(crate::project::FILE)).ok()?;
    let (wanted, unlabelled) = match wanted_labelled(ctx, &directory, &text) {
        Ok(wanted) => (wanted, None),
        Err((wanted, why)) => (wanted, Some(why)),
    };
    Some((text, wanted, unlabelled))
}

/// Whether the image `one` names is on this machine another than the one `container` runs.
async fn moved(ctx: &Ctx, container: &Container, one: &Wanted) -> bool {
    let Some(reference) = one.image.as_deref() else {
        return false;
    };
    matches!(
        ctx.seams.images.identified(reference).await,
        Ok(Some(id)) if id != container.image_id
    )
}

/// Connect `container` to each network `one` joins, again where the names it answers to there
/// or its address there changed, and disconnect it from each of `owned` it no longer joins;
/// whether the engine did each.
async fn joined(ctx: &Ctx, container: &Container, one: &Wanted, owned: &BTreeSet<String>) -> bool {
    let on: Vec<&crate::project::document::Joined> = one
        .networks
        .iter()
        .filter(|(network, ..)| container.networks.contains(network))
        .collect();
    let standing = if on.is_empty() {
        Vec::new()
    } else {
        let Ok(standing) = ctx.seams.networks.endpoints(&container.id).await else {
            return false;
        };
        standing
    };
    let mut done = true;
    for (network, aliases, address) in &one.networks {
        if on.iter().any(|(joined, ..)| joined == network) {
            if standing
                .iter()
                .find(|endpoint| endpoint.network == *network)
                .is_none_or(|endpoint| holds(endpoint, aliases, address.as_deref()))
            {
                continue;
            }
            done &= ctx
                .seams
                .networks
                .disconnected(network, &container.id)
                .await
                .is_ok();
        }
        let joining = Joining {
            network,
            container: &container.id,
            aliases,
            address: address.as_deref(),
        };
        done &= ctx.seams.networks.connected(joining).await.is_ok();
    }
    for network in container.networks.iter().filter(|network| {
        owned.contains(*network) && !one.networks.iter().any(|(name, ..)| name == *network)
    }) {
        done &= ctx
            .seams
            .networks
            .disconnected(network, &container.id)
            .await
            .is_ok();
    }
    done
}

/// Whether `endpoint` answers to exactly `aliases` and holds `address`.
fn holds(endpoint: &Endpoint, aliases: &[String], address: Option<&str>) -> bool {
    let held: BTreeSet<&str> = endpoint.aliases.iter().map(String::as_str).collect();
    let wanted: BTreeSet<&str> = aliases.iter().map(String::as_str).collect();
    endpoint.address.as_deref() == address && held == wanted
}

/// Write the labels file beneath `directory` for `document` and the operator's overlays.
///
/// # Errors
///
/// Where the file cannot be written.
pub(super) fn labelled(ctx: &Ctx, directory: &Path, document: &str) -> Result<(), String> {
    wanted_labelled(ctx, directory, document)
        .map(|_| ())
        .map_err(|(_, why)| why)
}

/// What each service `document` and the operator's overlays declare wants, its digest
/// written to the labels file beneath `directory`, or with why that file could not be
/// written.
fn wanted_labelled(
    ctx: &Ctx,
    directory: &Path,
    document: &str,
) -> Result<BTreeMap<String, Wanted>, (BTreeMap<String, Wanted>, String)> {
    let overlays: Vec<String> = ctx
        .settings
        .overlays
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect();
    let wanted = wanted(document, &overlays, directory, &ctx.settings.project);
    match super::fencing::kept(&directory.join(crate::project::LABELS), &labels(&wanted)) {
        Ok(()) => Ok(wanted),
        Err(why) => Err((wanted, why)),
    }
}

/// What `document` and `overlays` want of each service they declare, its secrets files read
/// beneath `directory`, the networks named as the engine names them for `project`.
fn wanted(
    document: &str,
    overlays: &[String],
    directory: &Path,
    project: &str,
) -> BTreeMap<String, Wanted> {
    let written = crate::project::document::written(document, overlays, project);
    let own: BTreeMap<&str, String> = written
        .iter()
        .map(|(service, one)| {
            let files: String = one
                .secrets_files
                .iter()
                .map(|file| std::fs::read_to_string(directory.join(file)).unwrap_or_default())
                .collect();
            (service.as_str(), one.entries.concat() + &files)
        })
        .collect();
    written
        .iter()
        .map(|(service, one)| {
            let wanted = Wanted {
                digest: sha256(&folded(service, &written, &own)),
                networks: one.networks.clone(),
                depends: one.depends.clone(),
                image: one.image.clone(),
                inside: one.inside.clone(),
            };
            (service.clone(), wanted)
        })
        .collect()
}

/// What `service`'s digest is taken over: its own, `own` says, then that of each service it
/// runs inside the network of in turn, so a service is created again with the one whose
/// network it runs inside.
fn folded(
    service: &str,
    written: &BTreeMap<String, crate::project::document::Written>,
    own: &BTreeMap<&str, String>,
) -> String {
    let mut seen = BTreeSet::new();
    let mut folded = String::new();
    let mut at = Some(service);
    while let Some(one) = at.filter(|one| seen.insert(*one)) {
        folded.push_str(own.get(one).map_or("", String::as_str));
        at = written
            .get(one)
            .and_then(|written| written.inside.as_deref());
    }
    folded
}

/// Each service a start of `scope` reaches, through every service each waits for, or every
/// service where `scope` is empty.
fn reached<'a>(wanted: &'a BTreeMap<String, Wanted>, scope: &'a [String]) -> BTreeSet<&'a str> {
    if scope.is_empty() {
        return wanted.keys().map(String::as_str).collect();
    }
    let mut reached = BTreeSet::new();
    let mut pending: Vec<&str> = scope.iter().map(String::as_str).collect();
    while let Some(service) = pending.pop() {
        if !reached.insert(service) {
            continue;
        }
        if let Some(one) = wanted.get(service) {
            pending.extend(one.depends.iter().map(String::as_str));
        }
    }
    reached
}

/// The labels file: each service labelled with its digest.
fn labels(wanted: &BTreeMap<String, Wanted>) -> String {
    let services: BTreeMap<&str, BTreeMap<&str, BTreeMap<&str, &str>>> = wanted
        .iter()
        .map(|(service, one)| {
            let labels = BTreeMap::from([(ENTRY_LABEL, one.digest.as_str())]);
            (service.as_str(), BTreeMap::from([("labels", labels)]))
        })
        .collect();
    crate::project::document::yaml(&BTreeMap::from([("services", services)]))
}

#[cfg(test)]
mod tests;
