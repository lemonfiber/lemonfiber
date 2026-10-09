//! What lemonfiber makes of one service asking for what another fills.
//!
//! Decided by what the asker speaks, what it asked for and what the filler speaks, and
//! for a curator by the media it files, which decides what it comes to in the asker.
//! Never by which services they are, so a plugin standing in for a bundled service and naming the
//! same adapter is connected exactly as the service it replaced was.
//!
//! **Every credential crosses one gate.** A connection hands the asker the filler's
//! credential, and the indexer's also hands the filler the asker's; each is made only
//! where [`crate::wiring::crosses`] lets that credential reach the one receiving it, so
//! no pass judges for itself whom a credential may reach.
//!
//! **A pair this table has no connection for is reported, never dropped.** Something
//! here fills what was asked for, and an operator reading a report that left it out
//! could not tell a filler nothing reaches from one lemonfiber forgot.

use lemonfiber_contract::capabilities::subtitles::fetch;
use lemonfiber_manifest::ApiKind;

use crate::ports::media::Kind;
use crate::ports::service::{ApplicationKind, Protocol};
use crate::seed::{State, Wiring};
use crate::wiring::{Address, Ask, Filler, Fillers};

/// Usenet downloading, which a media-filing \*arr asks for.
const USENET: &str = "download.usenet";

/// Torrent downloading, which a media-filing \*arr asks for.
const TORRENT: &str = "download.torrent";

/// Curating a library, which the indexer, the request service and the subtitle finder
/// each ask of every service that does it.
const CURATES: &str = "library.curate";

/// Searching across indexers, which the book \*arr asks of the service it pulls from.
const SEARCHES: &str = "indexer.search";

/// Television, as the stack manifest names the media a curator files.
pub(super) const TELEVISION: &str = Kind::Tv.media_type();
/// Film, likewise.
pub(super) const FILM: &str = Kind::Movies.media_type();
/// Music, likewise.
pub(super) const MUSIC: &str = ApplicationKind::Music.media_type();

/// Every capability an ask is answered for here.
///
/// An ask for anything else is connected where it always was, by the pass that wires
/// it, and is not reported here as reached by nothing.
const ANSWERED: [&str; 4] = [USENET, TORRENT, CURATES, SEARCHES];

/// Every contract an asker is paired by, as capability and major.
const CONTRACTED: [(&str, u32); 1] = [(fetch::CAPABILITY, fetch::MAJOR)];

/// An asker, as the table pairs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asking {
    /// A service this build holds an adapter for, of this kind.
    Bundled(ApiKind),
    /// A service speaking this capability's contract.
    Over(&'static str),
}

/// The asker as the table pairs it: by a contract it speaks before the adapter it names.
fn asking(asker: &Filler) -> Option<Asking> {
    CONTRACTED
        .iter()
        .find(|(capability, major)| asker.contracted(capability, *major))
        .map(|(capability, _)| Asking::Over(capability))
        .or_else(|| asker.adapter.as_ref().map(|api| Asking::Bundled(api.kind)))
}

/// What one asker and one filler come to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Connection {
    /// The filler, registered in the asker as a download client of this kind.
    DownloadClient(Protocol),
    /// The filler, registered in the indexer as an application of this kind, so the
    /// indexer pushes it what it searches.
    Application(ApplicationKind),
    /// The filler, handed the requests the request service accepts for the kind of video
    /// it fetches.
    Fulfilment {
        /// The kind of video it fetches.
        kind: Kind,
    },
    /// The filler, watched by the subtitle finder as this kind.
    Subtitles(Kind),
    /// The filler, told to the asker as an aggregator it pulls indexers from, with the
    /// filler's own key to read it with.
    Aggregator,
}

impl Connection {
    /// Whether making it hands the filler the asker's own credential.
    ///
    /// The indexer gives every application it syncs its own API key, and that key opens
    /// every indexer it holds. Every other connection hands the asker the filler's
    /// credential, which is the filler's to give.
    const fn hands_over_the_askers_key(&self) -> bool {
        matches!(self, Self::Application(_))
    }
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
    /// The connection would hand the asker's own credential to a filler the gate does
    /// not let it reach.
    Withheld,
    /// Every connection here hands the asker the filler's credential, and the gate does
    /// not let it reach this asker.
    Asked,
}

/// An asker the gate let every credential its connection hands it reach.
///
/// Built only in [`made`], where the gate is asked, so whatever takes one — the
/// indexer's sync, the subtitle finder, anything that reads a credential to hand an
/// asker — is never handed a service the gate refused to give one to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Cleared<'a>(&'a Filler);

impl std::ops::Deref for Cleared<'_> {
    type Target = Filler;

    fn deref(&self) -> &Filler {
        self.0
    }
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
    /// The connection, with where the filler is reached for it and the asker as the gate
    /// cleared it, or why there is none.
    pub(super) made: Result<(Connection, &'a Address, Cleared<'a>), Unmade>,
}

/// The connection the table holds for `asker`, asking for `capability`, of a filler
/// speaking `filler`.
///
/// A curator comes to whatever its media makes it in the asker, through the same
/// mapping each asker's own pass uses.
fn connection(
    asker: Asking,
    capability: &str,
    filler: ApiKind,
    media: &[String],
) -> Result<Connection, Unmade> {
    match (asker, capability, filler) {
        (Asking::Bundled(ApiKind::Servarr), USENET, ApiKind::Sabnzbd)
        | (Asking::Bundled(ApiKind::Servarr), TORRENT, ApiKind::Qbittorrent) => Ok(
            Connection::DownloadClient(Protocol(filler.name().to_owned())),
        ),
        (Asking::Bundled(ApiKind::Servarr), CURATES, ApiKind::Servarr) => {
            super::applications::application_kind(media)
                .map(Connection::Application)
                .ok_or(Unmade::Files)
        }
        (Asking::Bundled(ApiKind::Seerr), CURATES, ApiKind::Servarr) => Kind::of_declared(media)
            .map(|kind| Connection::Fulfilment { kind })
            .ok_or(Unmade::Files),
        (
            Asking::Bundled(ApiKind::Bazarr) | Asking::Over(fetch::CAPABILITY),
            CURATES,
            ApiKind::Servarr,
        ) => Kind::of_declared(media)
            .map(Connection::Subtitles)
            .ok_or(Unmade::Files),
        (Asking::Bundled(ApiKind::Bindery), SEARCHES, ApiKind::Servarr) => {
            Ok(Connection::Aggregator)
        }
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
    asker: &'a Filler,
    ask: &Ask,
    filler: &'a Filler,
) -> Result<(Connection, &'a Address, Cleared<'a>), Unmade> {
    // Every connection here hands the asker the filler's credential, so an asker that
    // credential may not reach is connected to nothing, whatever fills what it asked for.
    if !crate::wiring::crosses(filler.holder(), asker.holder()) {
        return Err(Unmade::Asked);
    }
    let speaks = filler.adapter.as_ref().ok_or(Unmade::NoAdapter)?.kind;
    let at = filler.address.as_ref().ok_or(Unmade::NoPort)?;
    let asks = asking(asker).ok_or(Unmade::Unpaired)?;
    let made = connection(asks, &ask.capability, speaks, &filler.media_types)?;
    if made.hands_over_the_askers_key() && !crate::wiring::crosses(asker.holder(), filler.holder())
    {
        return Err(Unmade::Withheld);
    }
    Ok((made, at, Cleared(asker)))
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
            let why = *pairing.made.as_ref().err()?;
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
        Unmade::Withheld => format!(
            "{fills} and is a third-party plugin's service; {asker} hands every service it \
             registers its own API key, which opens every indexer it holds, so lemonfiber does \
             not register a third-party plugin's service in it"
        ),
        Unmade::Asked => format!(
            "{fills} and {asker} is a third-party plugin's service, which lemonfiber never \
             hands another service's credential"
        ),
    }
}

#[cfg(test)]
mod tests;
