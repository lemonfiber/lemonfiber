//! What lemonfiber makes of one service asking for what another fills.
//!
//! Decided by three things and nothing else: what the asker speaks, what it asked for,
//! and what the filler speaks. Never by which services they are, so a plugin standing in
//! for a bundled service and naming the same adapter is connected exactly as the service
//! it replaced was.
//!
//! **A pair this table has no connection for is reported, never dropped.** Something
//! here fills what was asked for, and an operator reading a report that left it out
//! could not tell a filler nothing reaches from one lemonfiber forgot.

use lemonfiber_manifest::ApiKind;

use crate::ports::service::ClientKind;
use crate::seed::{State, Wiring};
use crate::wiring::{Address, Ask, Filler, Fillers};

/// Usenet downloading, which a media-filing \*arr asks for.
const USENET: &str = "download.usenet";

/// Torrent downloading, which a media-filing \*arr asks for.
const TORRENT: &str = "download.torrent";

/// Every capability an ask is answered for here.
///
/// An ask for anything else is connected where it always was, by the pass that wires
/// it, and is not reported here as reached by nothing.
const ANSWERED: [&str; 2] = [USENET, TORRENT];

/// What one asker and one filler come to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Connection {
    /// The filler, registered in the asker as a download client of this kind.
    DownloadClient(ClientKind),
}

/// Why one asker and one filler come to nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Unmade {
    /// The filler names no adapter, so there is nothing lemonfiber speaks to it through.
    NoAdapter,
    /// The filler names an adapter and no port it answers on beside the others.
    NoPort,
    /// Nothing in this table connects what the asker speaks to what the filler speaks.
    Unpaired,
}

/// One asker, one of the services that fills what it asked for, and what lemonfiber
/// makes of the two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pairing<'a> {
    /// The service that asks.
    pub(super) asker: &'a Filler,
    /// What it asked for.
    pub(super) ask: &'a Ask,
    /// One service that answers.
    pub(super) filler: &'a Filler,
    /// The connection, with where the filler is reached for it, or why there is none.
    pub(super) made: Result<(Connection, &'a Address), Unmade>,
}

/// The connection the table holds for an asker speaking `asker`, asking for
/// `capability`, of a filler speaking `filler`.
fn connection(asker: ApiKind, capability: &str, filler: ApiKind) -> Option<Connection> {
    match (asker, capability, filler) {
        (ApiKind::Servarr, USENET, ApiKind::Sabnzbd) => {
            Some(Connection::DownloadClient(ClientKind::Sabnzbd))
        }
        (ApiKind::Servarr, TORRENT, ApiKind::Qbittorrent) => {
            Some(Connection::DownloadClient(ClientKind::Qbittorrent))
        }
        _ => None,
    }
}

/// Every asker paired with each service that fills what it asked for, for the asks
/// answered here.
///
/// An ask whose asker is not on this machine is passed over: the operator took it out of
/// lemonfiber's hands, and a pass that wrote nothing to it has nothing to say about it.
pub(super) fn pairings(fillers: &Fillers) -> Vec<Pairing<'_>> {
    let mut found = Vec::new();
    for ask in fillers.asks() {
        if !ANSWERED.contains(&ask.capability.as_str()) {
            continue;
        }
        let Some(asker) = fillers.service(&ask.by) else {
            continue;
        };
        for filler in &ask.fillers {
            found.push(Pairing {
                asker,
                ask,
                filler,
                made: made(asker, ask, filler),
            });
        }
    }
    found
}

/// What one asker and one filler come to.
fn made<'a>(
    asker: &Filler,
    ask: &Ask,
    filler: &'a Filler,
) -> Result<(Connection, &'a Address), Unmade> {
    let speaks = filler.adapter.as_ref().ok_or(Unmade::NoAdapter)?.kind;
    let at = filler.address.as_ref().ok_or(Unmade::NoPort)?;
    asker
        .adapter
        .as_ref()
        .and_then(|api| connection(api.kind, &ask.capability, speaks))
        .map(|connection| (connection, at))
        .ok_or(Unmade::Unpaired)
}

/// Every pairing that comes to nothing, each reported naming what fills and what asked.
pub(super) fn unmatched(fillers: &Fillers) -> Vec<Wiring> {
    pairings(fillers)
        .iter()
        .filter_map(|pairing| {
            let why = pairing.made.err()?;
            Some(Wiring::settled(
                format!("{} into {}", pairing.filler.name, pairing.asker.name),
                State::Unmatched {
                    reason: reason(pairing, why),
                },
            ))
        })
        .collect()
}

/// Why a pairing comes to nothing, in the operator's terms.
fn reason(pairing: &Pairing<'_>, why: Unmade) -> String {
    let (filler, asker) = (&pairing.filler.name, &pairing.asker.name);
    let fills = format!(
        "{filler} fills {}, which {asker} asks for,",
        pairing.ask.capability
    );
    match why {
        Unmade::NoAdapter => {
            format!("{fills} and names no adapter lemonfiber could tell {asker} about it through")
        }
        Unmade::NoPort => {
            format!("{fills} and does not say which port it answers on inside the stack's network")
        }
        Unmade::Unpaired => format!(
            "{fills} and nothing in lemonfiber tells {asker} about a service that speaks \
             {filler}'s adapter"
        ),
    }
}

#[cfg(test)]
mod tests;
