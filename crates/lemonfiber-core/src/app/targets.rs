//! Resolving stack services to the targets that reading a credential starts
//! from — the project root a config is read under, and the Servarr-shape
//! services whose credential can be proven. Seeding and diagnosis both begin
//! here, so the resolution lives in one place they can share.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::config::store;
use crate::doctor::credentials::Target;
use crate::stack::Source;

/// The directory Compose treats as the project root, where the services' config
/// volumes are bind-mounted — the same path `up` hands Compose as
/// `--project-directory`, resolved here without writing anything.
///
/// An external stack is its own root; an embedded one lives wherever it was
/// materialised. Without that path there is nowhere to read a service's key from,
/// which the caller turns into no targets rather than a guess.
pub(super) fn project_directory(stack: &Source, stack_dir: Option<&Path>) -> Option<PathBuf> {
    match stack {
        Source::External(path) => Some((*path).to_path_buf()),
        Source::Embedded(_) => stack_dir.map(Path::to_path_buf),
    }
}

/// The Servarr-shape services whose credential can be proven, and where to read
/// each one's key and reach it.
///
/// Only a service that speaks the Servarr shape, publishes a port to reach it on
/// and names the config file it writes its key to can be proven; anything else is
/// left out rather than reported as a fault. The host path to that file follows
/// the stack's bind-mount convention — a service's `/config` is `config/<id>`
/// under the project root — so the key the service wrote is read from where
/// Compose mounted it.
pub(super) fn servarr_targets(
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

/// The host path a service's config file is read from, per the stack's bind-mount
/// convention: a service's `/config` mount is `config/<id>` under the project root,
/// so its `api.path` of `/config/<inside>` is read from there. Nothing where the api
/// names no such path — the one place the convention is spelled, for every service
/// whose credential is read from disk.
fn config_path(
    project: &Path,
    service: &lemonfiber_manifest::Service,
    api_path: Option<&str>,
) -> Option<PathBuf> {
    let inside = api_path?.strip_prefix("/config/")?;
    Some(project.join("config").join(&service.id).join(inside))
}

/// One service as a target to prove, or nothing where it cannot be one.
pub(super) fn target_for(service: &lemonfiber_manifest::Service, project: &Path) -> Option<Target> {
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
pub(super) enum DownloadKind {
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
pub(super) struct DownloadTarget {
    /// Where to reach it on the host.
    pub base: String,
    /// Which client, so the caller picks the adapter, its credential and protocol.
    pub kind: DownloadKind,
}

/// The stack's download clients, resolved to host-side read targets.
///
/// Reached on the loopback the way a Servarr service is — `127.0.0.1:<port>`, the
/// host-published port — not the container-network address seeding uses to wire
/// one service to another. A client that publishes no port cannot be reached from
/// the host, and `SABnzbd` with nowhere to read its key from is no use to a read,
/// so either is left out rather than resolved to a target that cannot answer.
pub(super) fn download_targets(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<DownloadTarget> {
    services
        .iter()
        .filter_map(|service| download_target_for(service, project))
        .collect()
}

/// One download-client service as a read target, or nothing where it is not a
/// download client, cannot be reached, or (for `SABnzbd`) has no key to read.
fn download_target_for(
    service: &lemonfiber_manifest::Service,
    project: Option<&Path>,
) -> Option<DownloadTarget> {
    let api = service.api.as_ref()?;
    let port = service.port?;
    let kind = match api.kind {
        lemonfiber_manifest::ApiKind::Qbittorrent => DownloadKind::Qbittorrent,
        lemonfiber_manifest::ApiKind::Sabnzbd => DownloadKind::Sabnzbd {
            config: config_path(project?, service, api.path.as_deref())?,
        },
        _ => return None,
    };
    Some(DownloadTarget {
        base: format!("http://127.0.0.1:{port}"),
        kind,
    })
}

/// A secret lemonfiber minted and recorded in the environment file, read back by
/// its key, or nothing where none is recorded yet — an unreadable or absent file
/// reads the same as an empty value. The one reader for every credential lemonfiber
/// mints and records rather than reads from a service (qBittorrent's password,
/// Jellyfin's admin password), so the read-back stays identical across them.
pub(super) fn recorded_secret(ctx: &Ctx, key: &str) -> Option<String> {
    let path = ctx.settings.env_file.as_deref()?;
    let file = store::read(path).unwrap_or_default();
    let value = file.get(key)?;
    (!value.is_empty()).then(|| value.to_owned())
}

/// The qBittorrent web UI password recorded at seeding — read back for the
/// dashboard's transfers authentication and for a later seed run.
pub(super) fn recorded_qbittorrent_password(ctx: &Ctx) -> Option<String> {
    recorded_secret(ctx, crate::config::QBITTORRENT_PASSWORD_KEY)
}
