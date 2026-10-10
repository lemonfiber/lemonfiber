//! The download clients, and what they are carrying.
//!
//! Which clients this machine has, the stack's and every installed plugin's, how to
//! reach each from the host and what it answers to, and how many bytes they still owe
//! the disk — the figure a free-space finding projects exhaustion from.

use std::path::Path;

use lemonfiber_contract::capabilities::download::{torrent, usenet};
use lemonfiber_contract::Contracted;
use lemonfiber_manifest::{ApiKind, Manifest};

use crate::app::Ctx;
use crate::ports::filesystem::Beneath;
use crate::ports::service::Download;
use crate::qbittorrent::Qbittorrent;
use crate::sabnzbd::Sabnzbd;
use crate::wiring::{Filler, Fillers};

use super::filled::{spoken, Spoken};
use super::opening::{credential_file, loopback};
use super::secrets::{chosen_fillers, recorded_secret};
use crate::dashboard::Protocol;

/// How a download client is asked, and so which protocol its transfers move over.
pub(crate) enum DownloadKind {
    /// The bundled torrent client, reached with the web UI password recorded for it.
    Torrent {
        /// Where the host reaches it.
        base: String,
        /// The password recorded for this client alone.
        password: String,
    },
    /// The bundled Usenet client, reached with the key it wrote for itself.
    Usenet {
        /// Where the host reaches it.
        base: String,
        /// The key, read from the file this client's own declaration names.
        key: String,
    },
    /// A client asked over the contract of the protocol it moves.
    Over {
        /// The protocol it moves.
        protocol: Protocol,
        /// The client, over that protocol's contract.
        adapter: Contracted,
    },
}

/// A download client the host reads: which service it is, how it is asked, and whether
/// it is the one reached through the tunnel.
pub(crate) struct DownloadTarget {
    /// The id its container runs under.
    pub service: String,
    /// How it is asked.
    pub kind: DownloadKind,
    /// Whether its traffic goes through the tunnel, which makes it the one client the
    /// port the tunnel is granted belongs to.
    pub tunnelled: bool,
}

/// A download client, as the protocol it moves.
pub(crate) enum Downloading {
    /// A torrent client.
    Torrent(Box<dyn torrent::Fills>),
    /// A Usenet client.
    Usenet(Box<dyn usenet::Fills>),
}

impl DownloadTarget {
    /// The client, as the protocol it moves.
    pub(crate) fn client(&self, ctx: &Ctx) -> Downloading {
        match &self.kind {
            DownloadKind::Torrent { base, password } => Downloading::Torrent(Box::new(
                Qbittorrent::authenticated(ctx.seams.http.clone(), base, password.clone()),
            )),
            DownloadKind::Usenet { base, key } => Downloading::Usenet(Box::new(Sabnzbd::new(
                ctx.seams.http.clone(),
                base,
                key.clone(),
            ))),
            DownloadKind::Over {
                protocol: Protocol::Torrent,
                adapter,
            } => Downloading::Torrent(Box::new(torrent::Adapter(adapter.clone()))),
            DownloadKind::Over {
                protocol: Protocol::Usenet,
                adapter,
            } => Downloading::Usenet(Box::new(usenet::Adapter(adapter.clone()))),
        }
    }

    /// The client as a torrent client, where it is one.
    pub(crate) fn torrent(&self, ctx: &Ctx) -> Option<Box<dyn torrent::Fills>> {
        match self.client(ctx) {
            Downloading::Torrent(client) => Some(client),
            Downloading::Usenet(_) => None,
        }
    }

    /// The client as a Usenet client, where it is one.
    pub(crate) fn usenet(&self, ctx: &Ctx) -> Option<Box<dyn usenet::Fills>> {
        match self.client(ctx) {
            Downloading::Usenet(client) => Some(client),
            Downloading::Torrent(_) => None,
        }
    }
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
    declared_downloads(ctx, fillers)
        .await
        .into_iter()
        .filter_map(|declared| declared.target)
        .collect()
}

/// A download client this machine declares, by the id it runs under, with what reaches
/// it where anything does.
pub(crate) struct DeclaredDownload {
    /// The id its container runs under.
    pub id: String,
    /// Where the host reaches it and the credential it answers to, or nothing where it
    /// publishes no port or its credential is not in hand.
    pub target: Option<DownloadTarget>,
}

/// Every download client among `fillers`, in the order they are declared, whether or
/// not anything here can reach it.
///
/// For the requests that owe an answer about every client the stack runs: one that
/// could not be reached is named as not reached rather than left out, where a read
/// leaves it out because it has nothing to read. A client that publishes no port has
/// its credential left unread.
pub(crate) async fn declared_downloads(ctx: &Ctx, fillers: &Fillers) -> Vec<DeclaredDownload> {
    let mut declared = Vec::new();
    for filler in fillers.services() {
        let Some(protocol) = moving(filler) else {
            continue;
        };
        declared.push(DeclaredDownload {
            id: filler.id.clone(),
            target: reached(ctx, fillers, filler, protocol).await,
        });
    }
    declared
}

/// The protocol `filler` moves as a download client: by the contract it speaks, before
/// the bundled adapter it names. Nothing where it is no download client.
fn moving(filler: &Filler) -> Option<Protocol> {
    if filler.contracted(torrent::CAPABILITY, torrent::MAJOR) {
        Some(Protocol::Torrent)
    } else if filler.contracted(usenet::CAPABILITY, usenet::MAJOR) {
        Some(Protocol::Usenet)
    } else if filler.speaks(ApiKind::Qbittorrent) {
        Some(Protocol::Torrent)
    } else if filler.speaks(ApiKind::Sabnzbd) {
        Some(Protocol::Usenet)
    } else {
        None
    }
}

/// How the host asks `filler`, a download client moving `protocol`: over that
/// protocol's contract where it speaks it, otherwise as the bundled client at the port it
/// publishes with the credential it answers to. Nothing where it speaks the contract and
/// cannot be asked over it, publishes no port, or its credential is not in hand.
async fn reached(
    ctx: &Ctx,
    fillers: &Fillers,
    filler: &Filler,
    protocol: Protocol,
) -> Option<DownloadTarget> {
    let (capability, major) = match protocol {
        Protocol::Torrent => (torrent::CAPABILITY, torrent::MAJOR),
        Protocol::Usenet => (usenet::CAPABILITY, usenet::MAJOR),
    };
    let kind = match spoken(ctx, filler, capability, major).await {
        Spoken::Over(adapter) => DownloadKind::Over { protocol, adapter },
        Spoken::Unanswered => return None,
        Spoken::Not => answering(ctx, fillers, filler, &loopback(filler.published?)).await?,
    };
    Some(DownloadTarget {
        service: filler.id.clone(),
        kind,
        tunnelled: tunnelled(filler),
    })
}

/// The bundled client `filler` is, at `base`, with the credential it answers to, or
/// nothing where that credential is not in hand.
async fn answering(
    ctx: &Ctx,
    fillers: &Fillers,
    filler: &Filler,
    base: &str,
) -> Option<DownloadKind> {
    if filler.speaks(ApiKind::Qbittorrent) {
        return recorded_password(ctx, fillers, filler).map(|password| DownloadKind::Torrent {
            base: base.to_owned(),
            password,
        });
    }
    match usenet_key(ctx, filler).await {
        Beneath::Read(key) => Some(DownloadKind::Usenet {
            base: base.to_owned(),
            key,
        }),
        _ => None,
    }
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

/// The first torrent client among the targets.
///
/// `None` where there is none — a client lemonfiber cannot authenticate to is not a
/// target at all, and guessing at one would be worse than saying nothing.
pub(crate) fn torrent_client(
    ctx: &Ctx,
    targets: &[DownloadTarget],
) -> Option<Box<dyn torrent::Fills>> {
    targets.iter().find_map(|target| target.torrent(ctx))
}

/// The torrent client reached through the tunnel, as the bundled client holding its own
/// password: the one whose listening port the port the tunnel is granted belongs to.
///
/// `None` where no torrent client goes through the tunnel. A plugin's never does, so no
/// plugin's client is offered the forwarded port or read for it.
pub(crate) fn forwarded_client(ctx: &Ctx, targets: &[DownloadTarget]) -> Option<Qbittorrent> {
    targets
        .iter()
        .filter(|target| target.tunnelled)
        .find_map(|target| match &target.kind {
            DownloadKind::Torrent { base, password } => Some(Qbittorrent::authenticated(
                ctx.seams.http.clone(),
                base,
                password.clone(),
            )),
            DownloadKind::Usenet { .. } | DownloadKind::Over { .. } => None,
        })
}

/// One client's active downloads — nothing where it will not answer, so it is left out
/// rather than failing the read.
///
/// Shared by the dashboard's transfers panel (which keeps each client's protocol)
/// and the free-space projection (which only sums what is still to land), so the
/// two never read a client two different ways.
pub(crate) async fn read_transfers(ctx: &Ctx, target: &DownloadTarget) -> Vec<Download> {
    match target.client(ctx) {
        Downloading::Torrent(client) => client.transfers().await,
        Downloading::Usenet(client) => client.transfers().await,
    }
    .unwrap_or_default()
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
pub(crate) const fn protocol_of(kind: &DownloadKind) -> Protocol {
    match kind {
        DownloadKind::Torrent { .. } => Protocol::Torrent,
        DownloadKind::Usenet { .. } => Protocol::Usenet,
        DownloadKind::Over { protocol, .. } => *protocol,
    }
}

#[cfg(test)]
mod tests;
