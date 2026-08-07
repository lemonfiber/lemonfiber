//! Resolving stack services to the targets that reading a credential starts
//! from — the project root a config is read under, and the Servarr-shape
//! services whose credential can be proven. Seeding and diagnosis both begin
//! here, so the resolution lives in one place they can share.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::config::store;
use crate::doctor::credentials::Target;
use crate::jellyfin::Jellyfin;
use crate::ports::service::{Download, Transfers};
use crate::qbittorrent::Qbittorrent;
use crate::sabnzbd::Sabnzbd;
use crate::seerr::Seerr;
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
pub(super) fn config_path(
    project: &Path,
    service: &lemonfiber_manifest::Service,
    api_path: Option<&str>,
) -> Option<PathBuf> {
    let inside = api_path?.strip_prefix("/config/")?;
    Some(project.join("config").join(&service.id).join(inside))
}

/// A file kept beside the environment file — the one durable location the context
/// carries, so every record lemonfiber persists (the drift baseline, the materialised
/// checksums, the recorded quality choice) is placed the same way rather than each
/// re-deriving the directory. Nothing where no environment file is configured.
pub(super) fn beside_env(ctx: &Ctx, name: &str) -> Option<PathBuf> {
    let env = ctx.settings.env_file.as_deref()?;
    Some(env.with_file_name(name))
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

/// The write side of [`recorded_secret`]: record a credential lemonfiber minted
/// where a later run — and the dashboard — reads it back, or nowhere when there is
/// no environment file to keep it in. Best-effort, like the other records seeding
/// keeps: a run that cannot persist it still set the secret on the service.
pub(super) fn record_secret(ctx: &Ctx, key: &str, value: &str) {
    if let Some(path) = ctx.settings.env_file.as_deref() {
        let _ = store::set(path, key, value);
    }
}

/// The qBittorrent web UI password recorded at seeding — read back for the
/// dashboard's transfers authentication and for a later seed run.
pub(super) fn recorded_qbittorrent_password(ctx: &Ctx) -> Option<String> {
    recorded_secret(ctx, crate::config::QBITTORRENT_PASSWORD_KEY)
}

/// The operator's data root on the host, as the environment file records it — the
/// directory the stack's `/data` mount resolves to. Read so a service's root folder,
/// a path inside that mount, can be checked against the filesystem it actually files
/// into: a folder resolving to nothing on the host is a root folder that breaks the
/// stack. Nothing where no environment file names a data root.
pub(super) fn data_root(ctx: &Ctx) -> Option<std::path::PathBuf> {
    let path = ctx.settings.env_file.as_deref()?;
    let file = store::read(path).unwrap_or_default();
    crate::config::data_root_from_env(&file)
}

/// The household's Jellyfin as a reading client, for the last stage of a trace —
/// whether the item is finally in the library. Present only where the stack has a
/// Jellyfin and lemonfiber recorded the admin password it minted for it: the read signs
/// in with the household's own credential, so without it there is nothing to sign in as.
///
/// A trace treats its absence as one more thing it cannot tell rather than a fault, so
/// either gap simply leaves the availability question unanswered.
pub(super) fn jellyfin_reader(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<Jellyfin> {
    let addr = service_addr(services, lemonfiber_manifest::ApiKind::Jellyfin)?;
    let password = recorded_secret(ctx, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY)?;
    Some(Jellyfin::authenticated(
        ctx.http.clone(),
        addr.loopback,
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        password,
    ))
}

/// What reading the household's requests needs: the request service, and the media-server
/// credential the sign-in is made with. Seerr authenticates its household against Jellyfin,
/// so the account lemonfiber holds a password for is how it asks — as the owner, whose
/// session sees every member's requests.
pub(super) struct HouseholdAccess {
    /// The request service, reached on the host.
    pub seerr: Seerr,
    /// Where the request service reaches the media server, across the stack's own
    /// network — the sign-in names the server it authenticates against, and Seerr is the
    /// one doing the reaching, so this is its address for it rather than the host's.
    pub server_url: String,
    /// The media-server admin password lemonfiber minted and recorded.
    pub password: String,
}

/// The request service and the credential to read it with, or nothing where the stack has
/// no request service, no media server to authenticate against, or no recorded password
/// to sign in with.
///
/// The household view treats any of those as nothing to report rather than a fault: a
/// stack without a request service has no household requests, and one whose password was
/// never recorded has no way to ask for them.
pub(super) fn seerr_reader(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<HouseholdAccess> {
    let seerr = service_addr(services, lemonfiber_manifest::ApiKind::Seerr)?;
    let jellyfin = service_addr(services, lemonfiber_manifest::ApiKind::Jellyfin)?;
    let password = recorded_secret(ctx, crate::config::JELLYFIN_ADMIN_PASSWORD_KEY)?;
    Some(HouseholdAccess {
        seerr: Seerr::new(ctx.http.clone(), seerr.loopback, "seerr"),
        server_url: jellyfin.network_url,
        password,
    })
}

/// Where a service is reached — on the host, and across the stack's own network — the
/// two forms every service address is wanted in.
pub(super) struct ServiceAddr {
    /// The service's compose id: names the container, and is the host in a network URL.
    pub id: String,
    /// Where the host reaches it: `http://127.0.0.1:{port}`.
    pub loopback: String,
    /// Where another container reaches it across the stack network: `http://{id}:{port}`.
    pub network_url: String,
}

/// The address of the one service of a given api kind, or nothing where the stack has
/// none or it publishes no port to reach it on. The single place the "find the service
/// by its kind, format where it is reached" step lives, so every caller that speaks to a
/// named service resolves it the same way rather than re-deriving the URLs.
pub(super) fn service_addr(
    services: &[lemonfiber_manifest::Service],
    kind: lemonfiber_manifest::ApiKind,
) -> Option<ServiceAddr> {
    services.iter().find_map(|service| {
        let api = service.api.as_ref()?;
        if api.kind != kind {
            return None;
        }
        let port = service.port?;
        Some(ServiceAddr {
            id: service.id.clone(),
            loopback: format!("http://127.0.0.1:{port}"),
            network_url: format!("http://{}:{port}", service.id),
        })
    })
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
