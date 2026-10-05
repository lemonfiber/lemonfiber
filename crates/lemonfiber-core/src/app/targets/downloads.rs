//! The download clients, and what they are carrying.
//!
//! Which clients this machine has, the stack's and every installed plugin's, how to
//! reach each from the host and what it answers to, and how many bytes they still owe
//! the disk — the figure a free-space finding projects exhaustion from.

use std::path::Path;

use lemonfiber_manifest::{ApiKind, Manifest};

use crate::app::Ctx;
use crate::ports::filesystem::Beneath;
use crate::ports::service::{Download, Transfers};
use crate::qbittorrent::Qbittorrent;
use crate::sabnzbd::Sabnzbd;
use crate::wiring::{Filler, Fillers};

use super::opening::{credential_file, loopback};
use super::secrets::{chosen_fillers, recorded_secret};
use crate::dashboard::Protocol;

/// Which download client a target is, with the credential that reaches it — and so
/// which protocol its transfers move over.
pub(crate) enum DownloadKind {
    /// qBittorrent: torrents, reached with the web UI password recorded for this client.
    Qbittorrent {
        /// The password recorded for this client alone.
        password: String,
    },
    /// `SABnzbd`: Usenet, reached with the key it wrote for itself.
    Sabnzbd {
        /// The key, read from the file this client's own declaration names.
        key: String,
    },
}

/// A download client the host reads: where to reach it, which client it is with what
/// it answers to, and whether it is the one reached through the tunnel.
pub(crate) struct DownloadTarget {
    /// Where to reach it on the host.
    pub base: String,
    /// Which client, so the caller picks the adapter and its protocol.
    pub kind: DownloadKind,
    /// Whether its traffic goes through the tunnel, which makes it the one client the
    /// port the tunnel is granted belongs to.
    pub tunnelled: bool,
}

/// Every service on this machine as a read from the host reaches it: the stack's, and
/// each installed plugin's where the record of what is installed reads.
///
/// A record that will not read narrows the answer to the stack's own services rather
/// than failing the read: what these reads report is made from what could be read, and
/// a client left out of it is said nowhere to be there.
pub(crate) fn host_fillers(ctx: &Ctx, manifest: &Manifest, project: Option<&Path>) -> Fillers {
    let register =
        crate::app::plugins::read(ctx).unwrap_or_else(|_| crate::plugin::Register::empty());
    Fillers::of(
        manifest,
        register.installed(),
        &chosen_fillers(ctx),
        project,
    )
}

/// The download clients among `fillers`, in the order they are declared, each resolved
/// to where the host reaches it and the credential it answers to.
///
/// Reached on the loopback at the port it publishes, not at the address services use
/// to reach one another. Each is proved with its own credential, read as seeding reads
/// it: a torrent client's password from the setting kept for that client alone, so one
/// a plugin brought is never sent the stack's, and a Usenet client's key from the file
/// its own declaration names, beneath its own directory where a plugin brought it. A
/// client that publishes no port, or whose credential is not in hand, is left out: a
/// read that cannot authenticate has nothing to read.
pub(crate) async fn download_targets(ctx: &Ctx, fillers: &Fillers) -> Vec<DownloadTarget> {
    let mut targets = Vec::new();
    for filler in fillers.services() {
        let Some(port) = filler.published else {
            continue;
        };
        let kind = if filler.speaks(ApiKind::Qbittorrent) {
            let Some(password) = recorded_password(ctx, fillers, filler) else {
                continue;
            };
            DownloadKind::Qbittorrent { password }
        } else if filler.speaks(ApiKind::Sabnzbd) {
            let Beneath::Read(key) = usenet_key(ctx, filler).await else {
                continue;
            };
            DownloadKind::Sabnzbd { key }
        } else {
            continue;
        };
        targets.push(DownloadTarget {
            base: loopback(port),
            kind,
            tunnelled: tunnelled(filler),
        });
    }
    targets
}

/// The password recorded for this torrent client, under the setting kept for it alone,
/// or nothing where none is recorded or no setting can be kept for it.
pub(crate) fn recorded_password(ctx: &Ctx, fillers: &Fillers, filler: &Filler) -> Option<String> {
    let setting = fillers.setting(filler, crate::config::PASSWORD_SUFFIX)?;
    recorded_secret(ctx, &setting)
}

/// The key this Usenet client wrote for itself, read from the file its own declaration
/// names — beneath its own directory where a plugin brought it.
///
/// [`Beneath::Read`] holds the key itself; a file holding none is as absent as one not
/// written, and a file refused stays refused, so the caller can say so.
pub(crate) async fn usenet_key(ctx: &Ctx, filler: &Filler) -> Beneath {
    match credential_file(ctx, filler).await {
        Beneath::Read(text) => {
            crate::sabnzbd::api_key(&text).map_or(Beneath::Absent, Beneath::Read)
        }
        other => other,
    }
}

/// Whether the service's traffic goes through the tunnel: it is reached at the address
/// of the service whose network it shares rather than at its own. A plugin's service
/// never is, since it cannot share another container's network.
fn tunnelled(filler: &Filler) -> bool {
    filler
        .address
        .as_ref()
        .is_some_and(|at| at.host != filler.id)
}

/// The first torrent client among the targets, as a client holding its own password.
///
/// `None` where there is none — a client lemonfiber cannot authenticate to is not a
/// target at all, and guessing at one would be worse than saying nothing.
pub(crate) fn torrent_client(ctx: &Ctx, targets: &[DownloadTarget]) -> Option<Qbittorrent> {
    targets.iter().find_map(|target| torrent(ctx, target))
}

/// The torrent client reached through the tunnel, as a client holding its own password:
/// the one whose listening port the port the tunnel is granted belongs to.
///
/// `None` where no torrent client goes through the tunnel. A plugin's never does, so no
/// plugin's client is offered the forwarded port or read for it.
pub(crate) fn forwarded_client(ctx: &Ctx, targets: &[DownloadTarget]) -> Option<Qbittorrent> {
    targets
        .iter()
        .filter(|target| target.tunnelled)
        .find_map(|target| torrent(ctx, target))
}

/// The target as a torrent client holding its own password, where it is one.
fn torrent(ctx: &Ctx, target: &DownloadTarget) -> Option<Qbittorrent> {
    match &target.kind {
        DownloadKind::Qbittorrent { password } => Some(Qbittorrent::authenticated(
            ctx.seams.http.clone(),
            &target.base,
            password.clone(),
        )),
        DownloadKind::Sabnzbd { .. } => None,
    }
}

/// One client's active downloads, read on its own shape — nothing where it will not
/// answer, so it is left out rather than failing the read.
///
/// Shared by the dashboard's transfers panel (which keeps each client's protocol)
/// and the free-space projection (which only sums what is still to land), so the
/// two never read a client two different ways.
pub(crate) async fn read_transfers(ctx: &Ctx, target: &DownloadTarget) -> Vec<Download> {
    match &target.kind {
        DownloadKind::Qbittorrent { password } => {
            Qbittorrent::authenticated(ctx.seams.http.clone(), &target.base, password.clone())
                .transfers()
                .await
                .unwrap_or_default()
        }
        DownloadKind::Sabnzbd { key } => {
            Sabnzbd::new(ctx.seams.http.clone(), &target.base, key.clone())
                .transfers()
                .await
                .unwrap_or_default()
        }
    }
}

/// The bytes the download clients among `fillers` still have to write, summed across
/// every client that answers — the committed content the free-space check projects
/// exhaustion from.
///
/// A client that will not answer, or a machine with no download client at all,
/// contributes nothing: the figure is made from what could be read. Zero and "no
/// clients" are one and the same here — both leave nothing to subtract from the
/// free space — so the projection reads as a plain `0` rather than an absence the
/// caller would have to fold back to zero anyway.
pub(crate) async fn committed_bytes(ctx: &Ctx, fillers: &Fillers) -> u64 {
    let mut downloads = Vec::new();
    for target in &download_targets(ctx, fillers).await {
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

/// The protocol a client's transfers move over.
pub(crate) fn protocol_of(kind: &DownloadKind) -> Protocol {
    match kind {
        DownloadKind::Qbittorrent { .. } => Protocol::Torrent,
        DownloadKind::Sabnzbd { .. } => Protocol::Usenet,
    }
}

#[cfg(test)]
mod tests;
