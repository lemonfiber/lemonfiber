//! Resolving stack services to the targets that reading a credential starts
//! from — the project root a config is read under, and the Servarr-shape
//! services whose credential can be proven. Seeding and diagnosis both begin
//! here, so the resolution lives in one place they can share.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::config::store;
use crate::doctor::credentials::Target;
use crate::ports::service::{Download, Transfers};
use crate::qbittorrent::Qbittorrent;
use crate::sabnzbd::Sabnzbd;
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

/// One client's active downloads, read on its own shape — nothing where it is not
/// yet seeded or will not answer, so it is left out rather than failing the read.
///
/// Shared by the dashboard's transfers panel (which keeps each client's protocol)
/// and the free-space projection (which only sums what is still to land), so the
/// two never read a client two different ways.
pub(super) async fn read_transfers(ctx: &Ctx, target: &DownloadTarget) -> Vec<Download> {
    match &target.kind {
        DownloadKind::Qbittorrent => {
            let Some(password) = recorded_qbittorrent_password(ctx) else {
                return Vec::new();
            };
            Qbittorrent::authenticated(ctx.http.clone(), &target.base, password)
                .transfers()
                .await
                .unwrap_or_default()
        }
        DownloadKind::Sabnzbd { config } => {
            let Some(text) = ctx.filesystem.read(config).await else {
                return Vec::new();
            };
            let Some(key) = crate::sabnzbd::api_key(&text) else {
                return Vec::new();
            };
            Sabnzbd::new(ctx.http.clone(), &target.base, key)
                .transfers()
                .await
                .unwrap_or_default()
        }
    }
}

/// The bytes the stack's download clients still have to write, summed across every
/// client that answers — the committed content the free-space check projects
/// exhaustion from.
///
/// A client that will not answer, or a stack with no download client at all,
/// contributes nothing: the figure is made from what could be read. Zero and "no
/// clients" are one and the same here — both leave nothing to subtract from the
/// free space — so the projection reads as a plain `0` rather than an absence the
/// caller would have to fold back to zero anyway.
pub(super) async fn committed_bytes(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> u64 {
    let mut downloads = Vec::new();
    for target in &download_targets(services, project) {
        downloads.extend(read_transfers(ctx, target).await);
    }
    committed_of(&downloads)
}

/// The bytes a set of active downloads still have to write, saturating so no sum of
/// client figures can wrap. A download whose client reported no figure contributes
/// nothing, kept apart from one reporting zero left — the pure half of
/// [`committed_bytes`], decided without touching a client.
fn committed_of(downloads: &[Download]) -> u64 {
    downloads
        .iter()
        .filter_map(|download| download.remaining)
        .fold(0, u64::saturating_add)
}

#[cfg(test)]
mod tests {
    use super::committed_of;
    use crate::ports::service::Download;

    /// A download with only its bytes-still-to-write set — the one field the
    /// committed sum reads.
    fn download(remaining: Option<u64>) -> Download {
        Download {
            name: "item".to_owned(),
            progress: 0,
            speed: None,
            eta: None,
            remaining,
        }
    }

    #[test]
    fn committed_sums_what_each_download_still_has_to_write() {
        let downloads = [download(Some(300)), download(Some(200))];
        assert_eq!(committed_of(&downloads), 500);
    }

    #[test]
    fn a_download_reporting_no_figure_is_left_out_of_the_sum() {
        // An unknown is not counted as zero-left; it is simply left out, so a
        // client that reports no figure never reads as "nothing more to write".
        let downloads = [download(Some(200)), download(None)];
        assert_eq!(committed_of(&downloads), 200);
    }

    #[test]
    fn the_sum_saturates_rather_than_wrapping() {
        let downloads = [download(Some(u64::MAX)), download(Some(1))];
        assert_eq!(committed_of(&downloads), u64::MAX);
    }
}
