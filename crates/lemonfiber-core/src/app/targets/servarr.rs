//! The Servarr-shape services whose credential can be proven.
//!
//! Only a service that speaks the shape, publishes a port and names the file it writes its
//! key to can be a target; anything else is left out rather than reported as a fault.

use std::path::{Path, PathBuf};

use crate::doctor::credentials::Target;

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
