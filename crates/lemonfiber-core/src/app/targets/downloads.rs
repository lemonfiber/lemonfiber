//! The download clients, and what they are carrying.
//!
//! Which clients this machine has, the stack's and every installed plugin's, the contract
//! each is asked over, and how many bytes they still owe the disk — the figure a
//! free-space finding projects exhaustion from.

use std::path::Path;

use lemonfiber_contract::capabilities::download::{torrent, usenet};
use lemonfiber_contract::Contracted;
use lemonfiber_manifest::Manifest;

use crate::app::Ctx;
use crate::ports::service::Download;
use crate::wiring::{Filler, Fillers};

use super::filled::{spoken, Spoken};
use super::secrets::chosen_fillers;
use crate::dashboard::Protocol;

/// A download client the host reads: which service it is, the protocol its transfers
/// move over, and that protocol's contract it is asked over.
pub(crate) struct DownloadTarget {
    /// The id its container runs under.
    pub service: String,
    /// The protocol it moves.
    pub protocol: Protocol,
    /// The client, over that protocol's contract.
    pub adapter: Contracted,
}

/// A download client, as the protocol it moves.
pub(crate) enum Downloading {
    /// A torrent client.
    Torrent(Box<dyn torrent::Fills>),
    /// A Usenet client.
    Usenet(Box<dyn usenet::Fills>),
}

impl Downloading {
    /// The client, as it is asked where each category's completed downloads land.
    pub(crate) fn filing(&self) -> &dyn crate::ports::service::Filing {
        match self {
            Self::Torrent(client) => client.as_ref(),
            Self::Usenet(client) => client.as_ref(),
        }
    }
}

impl DownloadTarget {
    /// The client, as the protocol it moves.
    pub(crate) fn client(&self) -> Downloading {
        let adapter = self.adapter.clone();
        match self.protocol {
            Protocol::Torrent => Downloading::Torrent(Box::new(torrent::Adapter(adapter))),
            Protocol::Usenet => Downloading::Usenet(Box::new(usenet::Adapter(adapter))),
        }
    }

    /// The client as a torrent client, where it is one.
    pub(crate) fn torrent(&self) -> Option<Box<dyn torrent::Fills>> {
        match self.client() {
            Downloading::Torrent(client) => Some(client),
            Downloading::Usenet(_) => None,
        }
    }

    /// The client as a Usenet client, where it is one.
    pub(crate) fn usenet(&self) -> Option<Box<dyn usenet::Fills>> {
        match self.client() {
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

/// The download clients among `fillers`, in the order they are declared, each asked over
/// its protocol's contract with its own key.
///
/// One that cannot be asked over it is left out: a read that cannot reach a client has
/// nothing to read.
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
    /// How the host asks it, or nothing where it cannot be asked over its protocol's
    /// contract.
    pub target: Option<DownloadTarget>,
}

/// Every download client among `fillers`, in the order they are declared, whether or
/// not anything here can reach it.
///
/// For the requests that owe an answer about every client the stack runs: one that
/// could not be reached is named as not reached rather than left out, where a read
/// leaves it out because it has nothing to read.
pub(crate) async fn declared_downloads(ctx: &Ctx, fillers: &Fillers) -> Vec<DeclaredDownload> {
    let mut declared = Vec::new();
    for filler in fillers.services() {
        let Some(moving) = moving(filler) else {
            continue;
        };
        declared.push(DeclaredDownload {
            id: filler.id.clone(),
            target: reached(ctx, filler, moving).await,
        });
    }
    declared
}

/// The protocols a download client moves, in the order one speaking both is taken as.
const PROTOCOLS: [Protocol; 2] = [Protocol::Torrent, Protocol::Usenet];

/// The capability and major of `protocol`'s contract.
const fn contract(protocol: Protocol) -> (&'static str, u32) {
    match protocol {
        Protocol::Torrent => (torrent::CAPABILITY, torrent::MAJOR),
        Protocol::Usenet => (usenet::CAPABILITY, usenet::MAJOR),
    }
}

/// The protocol `filler` moves as a download client: the one whose contract it speaks.
/// Nothing where it is no download client.
fn moving(filler: &Filler) -> Option<Protocol> {
    PROTOCOLS.into_iter().find(|protocol| {
        let (capability, major) = contract(*protocol);
        filler.contracted(capability, major)
    })
}

/// How the host asks `filler`, a download client moving `protocol`: over its contract.
/// Nothing where it cannot be asked over it.
async fn reached(ctx: &Ctx, filler: &Filler, protocol: Protocol) -> Option<DownloadTarget> {
    let (capability, major) = contract(protocol);
    match spoken(ctx, filler, capability, major).await {
        Spoken::Over(adapter) => Some(DownloadTarget {
            service: filler.id.clone(),
            protocol,
            adapter,
        }),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// The first torrent client among the targets.
///
/// `None` where there is none — a client lemonfiber cannot ask is not a target at all,
/// and guessing at one would be worse than saying nothing.
pub(crate) fn torrent_client(targets: &[DownloadTarget]) -> Option<Box<dyn torrent::Fills>> {
    targets.iter().find_map(DownloadTarget::torrent)
}

/// One client's active downloads — nothing where it will not answer, so it is left out
/// rather than failing the read.
///
/// Shared by the dashboard's transfers panel (which keeps each client's protocol)
/// and the free-space projection (which only sums what is still to land), so the
/// two never read a client two different ways.
pub(crate) async fn read_transfers(target: &DownloadTarget) -> Vec<Download> {
    match target.client() {
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
        downloads.extend(read_transfers(target).await);
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

#[cfg(test)]
mod tests;
