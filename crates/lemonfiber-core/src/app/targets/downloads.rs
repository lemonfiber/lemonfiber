//! The download clients, and what they are carrying.
//!
//! Which clients the stack has, how to reach them, and how many bytes they still owe the
//! disk — the figure a free-space finding projects exhaustion from.

use std::path::Path;

use crate::app::Ctx;
use crate::ports::service::{Download, Transfers};
use crate::qbittorrent::Qbittorrent;
use crate::sabnzbd::Sabnzbd;

use super::layout::*;
use super::secrets::*;
use super::servarr::*;

/// The stack's download clients, resolved to host-side read targets.
///
/// Reached on the loopback the way a Servarr service is — `127.0.0.1:<port>`, the
/// host-published port — not the container-network address seeding uses to wire
/// one service to another. A client that publishes no port cannot be reached from
/// the host, and `SABnzbd` with nowhere to read its key from is no use to a read,
/// so either is left out rather than resolved to a target that cannot answer.
pub(crate) fn download_targets(
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

/// One client's active downloads, read on its own shape — nothing where it is not
/// yet seeded or will not answer, so it is left out rather than failing the read.
///
/// Shared by the dashboard's transfers panel (which keeps each client's protocol)
/// and the free-space projection (which only sums what is still to land), so the
/// two never read a client two different ways.
/// The torrent client's listening port, and a way to change it.
///
/// `None` where this stack has no torrent client, or where lemonfiber has no
/// recorded password for it — a client it cannot authenticate to is one it cannot
/// read or correct, and guessing either way would be worse than saying nothing.
pub(crate) fn torrent_client(ctx: &Ctx, targets: &[DownloadTarget]) -> Option<Qbittorrent> {
    let target = targets
        .iter()
        .find(|target| matches!(target.kind, DownloadKind::Qbittorrent))?;
    let password = recorded_qbittorrent_password(ctx)?;
    Some(Qbittorrent::authenticated(
        ctx.http.clone(),
        &target.base,
        password,
    ))
}

pub(crate) async fn read_transfers(ctx: &Ctx, target: &DownloadTarget) -> Vec<Download> {
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
pub(crate) async fn committed_bytes(
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
pub(crate) fn committed_of(downloads: &[Download]) -> u64 {
    downloads
        .iter()
        .filter_map(|download| download.remaining)
        .fold(0, u64::saturating_add)
}
