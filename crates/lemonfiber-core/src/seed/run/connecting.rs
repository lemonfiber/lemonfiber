//! What lemonfiber makes of one service asking for what another fills.
//!
//! Decided by what the asker speaks, what it asked for and what the filler speaks, and
//! for a curator by the media it files, which decides what it comes to in the asker.
//! Never by which services they are, so a plugin standing in for a bundled service and naming the
//! same adapter is connected exactly as the service it replaced was.
//!
//! **A pair this table has no connection for is reported, never dropped.** Something
//! here fills what was asked for, and an operator reading a report that left it out
//! could not tell a filler nothing reaches from one lemonfiber forgot.

use lemonfiber_manifest::ApiKind;

use crate::ports::service::{ApplicationKind, ClientKind, Subtitled};
use crate::seed::{State, Wiring};
use crate::wiring::{Address, Ask, Filler, Fillers};

/// Usenet downloading, which a media-filing \*arr asks for.
const USENET: &str = "download.usenet";

/// Torrent downloading, which a media-filing \*arr asks for.
const TORRENT: &str = "download.torrent";

/// Curating a library, which the indexer, the request service and the subtitle finder
/// each ask of every service that does it.
const CURATES: &str = "library.curate";

/// Television, as the stack manifest names the media a curator files.
pub(super) const TELEVISION: &str = "tv";
/// Film, likewise.
pub(super) const FILM: &str = "movies";
/// Music, likewise.
pub(super) const MUSIC: &str = "music";

/// Every capability an ask is answered for here.
///
/// An ask for anything else is connected where it always was, by the pass that wires
/// it, and is not reported here as reached by nothing.
const ANSWERED: [&str; 3] = [USENET, TORRENT, CURATES];

/// What one asker and one filler come to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Connection {
    /// The filler, registered in the asker as a download client of this kind.
    DownloadClient(ClientKind),
    /// The filler, registered in the indexer as an application of this kind, so the
    /// indexer pushes it what it searches.
    Application(ApplicationKind),
    /// The filler, handed the requests the request service accepts for television where
    /// this is true and for film where it is not.
    Fulfilment {
        /// Whether it fetches television rather than film.
        television: bool,
    },
    /// The filler, watched by the subtitle finder as this kind.
    Subtitles(Subtitled),
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
    /// The two are connected for some media, and the filler files none of them.
    Files,
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
///
/// A curator comes to whatever its media makes it in the asker, through the same
/// mapping each asker's own pass uses.
fn connection(
    asker: ApiKind,
    capability: &str,
    filler: ApiKind,
    media: &[String],
) -> Result<Connection, Unmade> {
    match (asker, capability, filler) {
        (ApiKind::Servarr, USENET, ApiKind::Sabnzbd) => {
            Ok(Connection::DownloadClient(ClientKind::Sabnzbd))
        }
        (ApiKind::Servarr, TORRENT, ApiKind::Qbittorrent) => {
            Ok(Connection::DownloadClient(ClientKind::Qbittorrent))
        }
        (ApiKind::Servarr, CURATES, ApiKind::Servarr) => {
            super::applications::application_kind(media)
                .map(Connection::Application)
                .ok_or(Unmade::Files)
        }
        (ApiKind::Seerr, CURATES, ApiKind::Servarr) => super::fulfilment::fetches(media)
            .map(|television| Connection::Fulfilment { television })
            .ok_or(Unmade::Files),
        (ApiKind::Bazarr, CURATES, ApiKind::Servarr) => super::subtitles::subtitled(media)
            .map(Connection::Subtitles)
            .ok_or(Unmade::Files),
        _ => Err(Unmade::Unpaired),
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
    let asks = asker.adapter.as_ref().ok_or(Unmade::Unpaired)?.kind;
    connection(asks, &ask.capability, speaks, &filler.media_types).map(|made| (made, at))
}

/// Every pairing that comes to nothing, each reported naming what fills and what asked.
///
/// Except where the filler reaches the asker itself: it asks for something the asker
/// fills, so the two are connected from the filler's end, by the pass that answers that
/// ask, and saying the pair is reached by nothing would be wrong about it.
pub(super) fn unmatched(fillers: &Fillers) -> Vec<Wiring> {
    pairings(fillers)
        .iter()
        .filter(|pairing| !from_its_end(fillers, pairing))
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

/// Whether the filler asks for something the asker fills.
fn from_its_end(fillers: &Fillers, pairing: &Pairing<'_>) -> bool {
    fillers.asks().iter().any(|ask| {
        ask.by == pairing.filler.id && ask.fillers.iter().any(|one| one.id == pairing.asker.id)
    })
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
        Unmade::Files if pairing.filler.media_types.is_empty() => format!(
            "{fills} and names no media it files, so lemonfiber cannot say what to hand {asker}"
        ),
        Unmade::Files => format!(
            "{fills} and files {}, which lemonfiber does not hand {asker}",
            pairing.filler.media_types.join(" and ")
        ),
    }
}

#[cfg(test)]
mod tests;
