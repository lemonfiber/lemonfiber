//! The Servarr-shape services whose credential can be proven, and the ones that
//! declared the shape and cannot be reached through it.
//!
//! Only a service that speaks the shape, publishes a port and names the file it writes
//! its key to can be a target. A service that declares no API at all is left out
//! silently and rightly: the manifest saying nothing is the stack's own statement that
//! there is nothing here to integrate with, which is a different thing from lemonfiber
//! being unable to.
//!
//! A service that declares the shape and is missing a part of it is neither, and used
//! to be treated as the first. It is an operator — on their own stack, which is the
//! only place this arises — who wrote an API declaration and got no feature working
//! against it, with nothing anywhere saying why. That case is named now.

use std::path::{Path, PathBuf};

use crate::doctor::credentials::Target;
use crate::model::UnsupportedReport;

use super::layout::config_path;

/// The Servarr-shape services whose credential can be proven, and where to read
/// each one's key and reach it.
///
/// Only a service that speaks the Servarr shape, publishes a port to reach it on
/// and names the config file it writes its key to can be proven; anything else is
/// left out rather than reported as a fault. The host path to that file follows
/// the stack's bind-mount convention — a service's `/config` is `config/<id>`
/// under the project root — so the key the service wrote is read from where
/// Compose mounted it.
pub(crate) fn servarr_targets(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<Target> {
    let Some(project) = project else {
        return Vec::new();
    };
    services
        .iter()
        .filter_map(|service| target_for(service, project))
        .collect()
}

/// The one Servarr-shape service with this id, or nothing where the stack has none — the
/// lookup a reversal makes, which knows the name of the service it has to reach and
/// nothing else about it.
pub(crate) fn target_named(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
    id: &str,
) -> Option<Target> {
    servarr_targets(services, project)
        .into_iter()
        .find(|target| target.id == id)
}

/// One service as a target to prove, or nothing where it cannot be one.
pub(crate) fn target_for(service: &lemonfiber_manifest::Service, project: &Path) -> Option<Target> {
    let api = service.api.as_ref()?;
    if api.kind != lemonfiber_manifest::ApiKind::Servarr {
        return None;
    }
    let port = service.port?;
    let config = config_path(project, service, api.path.as_deref())?;
    // The Servarr shape spans two API versions, so the manifest carries it. A
    // servarr service that names none cannot be reached at a known path, so it is
    // no target rather than one guessed at the wrong version.
    let version = api.version?;
    Some(Target {
        id: service.id.clone(),
        name: service.name.clone(),
        base: format!("http://127.0.0.1:{port}"),
        config,
        version,
    })
}

/// Every service that declares the Servarr shape and cannot be reached through it,
/// each with what its declaration is missing.
///
/// Nothing where the stack has not been written to disk: the config file half of the
/// question is answered against a project root, and with none of them the answer
/// would be "unreachable" for every service at once — which says something about the
/// machine rather than about any declaration.
pub(crate) fn unreachable_targets(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<UnsupportedReport> {
    let Some(project) = project else {
        return Vec::new();
    };
    services
        .iter()
        .filter_map(|service| unreachable_for(service, project))
        .collect()
}

/// One service that declared the shape and cannot be reached through it, or nothing
/// where it can be — or where it never claimed to speak the shape at all.
fn unreachable_for(
    service: &lemonfiber_manifest::Service,
    project: &Path,
) -> Option<UnsupportedReport> {
    let api = service.api.as_ref()?;
    if api.kind != lemonfiber_manifest::ApiKind::Servarr {
        return None;
    }
    let because = missing_from(service, api, project)?;
    Some(UnsupportedReport {
        what: service.id.clone(),
        because,
    })
}

/// What a Servarr-shape declaration is missing, or nothing where it is complete.
///
/// In the order a reader would fix them, and one at a time: three sentences about one
/// service is a list nobody finishes, and the first of them is enough to act on.
fn missing_from(
    service: &lemonfiber_manifest::Service,
    api: &lemonfiber_manifest::Api,
    project: &Path,
) -> Option<String> {
    if service.port.is_none() {
        return Some(
            "it declares an API of the Servarr shape and no port to publish, so there is no \
             address to speak it on"
                .to_owned(),
        );
    }
    if api.version.is_none() {
        return Some(
            "the Servarr shape spans two API versions and this declaration names neither, so \
             the path to ask at would be a guess"
                .to_owned(),
        );
    }
    if config_path(project, service, api.path.as_deref()).is_none() {
        return Some(
            "it names no configuration file beneath a mount lemonfiber reads, so the key the \
             service writes for itself cannot be found"
                .to_owned(),
        );
    }
    None
}

/// Which download client a target is — and so which protocol its transfers move
/// over and which credential reaches it. `SABnzbd` carries the config file its key
/// is read from, so a resolved target always has one and the read never has to
/// check; qBittorrent carries nothing, reached with the recorded password instead.
pub(crate) enum DownloadKind {
    /// qBittorrent: torrents, reached with the recorded web UI password.
    Qbittorrent,
    /// `SABnzbd`: Usenet, reached with the key read from this config file.
    Sabnzbd {
        /// The config file `SABnzbd`'s key is read from.
        config: PathBuf,
    },
}

/// A download client the dashboard reads active transfers from: where to reach it
/// on the host, and which client it is.
pub(crate) struct DownloadTarget {
    /// Where to reach it on the host.
    pub base: String,
    /// Which client, so the caller picks the adapter, its credential and protocol.
    pub kind: DownloadKind,
}

#[cfg(test)]
mod tests;
