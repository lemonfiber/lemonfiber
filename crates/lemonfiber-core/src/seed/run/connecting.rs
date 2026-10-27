//! What lemonfiber makes of one service asking for what another fills.
//!
//! Decided by the contract the asker speaks, what it asked for and what the filler
//! speaks, and for a curator by the media it files, which decides what it comes to in the
//! asker. Never by which services they are, so any service speaking the same contract is
//! connected exactly as another that does.
//!
//! **Every credential crosses one gate.** A connection hands the asker the filler's
//! credential, and the indexer's also hands the filler the asker's; each is made only
//! where [`crate::wiring::crosses`] lets that credential reach the one receiving it, so
//! no pass judges for itself whom a credential may reach.
//!
//! **It answers for every asker speaking a contract it is paired by, and for no other.**
//! A curator speaking `library.curate` is told its download clients, one speaking
//! `indexer.pull` its aggregator, and one speaking `indexer.search`, `request.intake`,
//! `subtitles.fetch`, `quality.sync` or `archive.extract` the curators. An asker speaking
//! none of them — one of the stack's own services, or a plugin's that declares no
//! contract — is wired by the pass that reads what it needs, and that pass says where it
//! cannot: the credentials it reads out of another service's in [`super::crossing`], and
//! the request gate's routes in [`super::gate`].
//!
//! **A pair this table has no connection for is reported, never dropped.** Something
//! here fills what was asked for, and an operator reading a report that left it out
//! could not tell a filler nothing reaches from one lemonfiber forgot.

use lemonfiber_contract::capabilities::archive::extract;
use lemonfiber_contract::capabilities::download::{torrent, usenet};
use lemonfiber_contract::capabilities::indexer::{pull, search};
use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_contract::capabilities::quality::sync;
use lemonfiber_contract::capabilities::request::intake;
use lemonfiber_contract::capabilities::subtitles::fetch;
use lemonfiber_manifest::ApiKind;

use crate::ports::media::Kind;
use crate::ports::service::{ApplicationKind, Protocol};
use crate::seed::{State, Wiring};
use crate::wiring::{Address, Ask, Filler, Fillers, Holder};

/// Usenet downloading, which a media-filing curator asks for.
const USENET: &str = usenet::CAPABILITY;

/// Torrent downloading, which a media-filing curator asks for.
const TORRENT: &str = torrent::CAPABILITY;

/// Curating a library, which the indexer, the request service, the subtitle finder and the
/// quality sync each ask of every service that does it.
const CURATES: &str = curate::CAPABILITY;

/// Searching across indexers, which a curator pulling its indexers asks of the service it
/// pulls from.
const SEARCHES: &str = search::CAPABILITY;

/// Television, as the stack manifest names the media a curator files.
pub(super) const TELEVISION: &str = Kind::Tv.media_type();
/// Film, likewise.
pub(super) const FILM: &str = Kind::Movies.media_type();
/// Music, likewise.
pub(super) const MUSIC: &str = ApplicationKind::Music.media_type();
/// Books, likewise.
pub(super) const BOOKS: &str = crate::ports::media::BOOKS;

/// Every capability an ask is answered for here, where its asker speaks a contract it is
/// paired by.
///
/// An ask for anything else, and one whose asker speaks no such contract, is connected by
/// the pass that wires it, which reports where it cannot, and is not reported here as
/// reached by nothing.
const ANSWERED: [&str; 4] = [USENET, TORRENT, CURATES, SEARCHES];

/// Every contract an asker is paired by, each with the capability of the asks it is paired
/// by it for: a curator is told its download clients over `library.curate`, a curator
/// pulling its indexers its aggregator over `indexer.pull`, and each of the rest the
/// curators over its own.
const CONTRACTED: [(&str, &str, u32); 8] = [
    (USENET, curate::CAPABILITY, curate::MAJOR),
    (TORRENT, curate::CAPABILITY, curate::MAJOR),
    (SEARCHES, pull::CAPABILITY, pull::MAJOR),
    (CURATES, extract::CAPABILITY, extract::MAJOR),
    (CURATES, fetch::CAPABILITY, fetch::MAJOR),
    (CURATES, search::CAPABILITY, search::MAJOR),
    (CURATES, intake::CAPABILITY, intake::MAJOR),
    (CURATES, sync::CAPABILITY, sync::MAJOR),
];

/// The contract the asker is paired by for an ask of `capability`: one it speaks for asks
/// of that capability.
fn asking(asker: &Filler, capability: &str) -> Option<&'static str> {
    CONTRACTED
        .iter()
        .find(|(asked, contract, major)| *asked == capability && asker.contracted(contract, *major))
        .map(|(_, contract, _)| *contract)
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
    /// The filler, whose quality profiles the quality sync holds to the preset chosen for
    /// this kind.
    Quality(Kind),
    /// The filler, told to the asker as an aggregator it pulls indexers from, with the
    /// filler's own key to read it with.
    Aggregator,
    /// The filler, whose completed downloads the extractor unpacks and hands back to it.
    Extraction {
        /// The media it files.
        media: String,
        /// The category its downloads are filed under.
        category: String,
    },
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
    /// The filler names no API of its own, so there is nothing lemonfiber could tell the
    /// asker about it in.
    NoNative,
    /// The filler names an API of its own and no port it answers on beside the others.
    NoPort,
    /// Nothing in this table connects the contract the asker speaks to what the filler
    /// speaks.
    Unpaired,
    /// The filler's application sync serves the media the asker files, so the filler is
    /// told about the asker rather than the asker about the filler.
    Synced,
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
///
/// It reads as the service that asked, which every report names; [`Cleared::answering`]
/// is the service asked for it, the adapter in front of it where one of its plugin fronts
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Cleared<'a> {
    /// The service that asked.
    named: &'a Filler,
    /// The service asked for it.
    answering: &'a Filler,
}

impl<'a> Cleared<'a> {
    /// The service asked for the asker: the adapter that fronts it, or the asker itself.
    pub(super) const fn answering(&self) -> &'a Filler {
        self.answering
    }
}

impl std::ops::Deref for Cleared<'_> {
    type Target = Filler;

    fn deref(&self) -> &Filler {
        self.named
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

/// The connection the table holds for an asker paired by the contract `asker`, asking for
/// `capability`, of a filler speaking `filler`.
///
/// A curator comes to whatever its media makes it in the asker, through the same
/// mapping each asker's own pass uses.
fn connection(
    asker: &str,
    capability: &str,
    filler: ApiKind,
    media: &[String],
    files: &[String],
) -> Result<Connection, Unmade> {
    match (asker, capability, filler) {
        (curate::CAPABILITY, USENET, ApiKind::Sabnzbd)
        | (curate::CAPABILITY, TORRENT, ApiKind::Qbittorrent) => Ok(Connection::DownloadClient(
            Protocol(filler.name().to_owned()),
        )),
        (search::CAPABILITY, CURATES, ApiKind::Servarr) => {
            super::applications::application_kind(media)
                .map(Connection::Application)
                .ok_or(Unmade::Files)
        }
        (intake::CAPABILITY, CURATES, ApiKind::Servarr) => Kind::of_declared(media)
            .map(|kind| Connection::Fulfilment { kind })
            .ok_or(Unmade::Files),
        (fetch::CAPABILITY, CURATES, ApiKind::Servarr) => Kind::of_declared(media)
            .map(Connection::Subtitles)
            .ok_or(Unmade::Files),
        (sync::CAPABILITY, CURATES, ApiKind::Servarr) => Kind::of_declared(media)
            .map(Connection::Quality)
            .ok_or(Unmade::Files),
        (pull::CAPABILITY, SEARCHES, _)
            if super::applications::application_kind(files).is_some() =>
        {
            Err(Unmade::Synced)
        }
        (pull::CAPABILITY, SEARCHES, _) => Ok(Connection::Aggregator),
        (extract::CAPABILITY, CURATES, ApiKind::Servarr) => media
            .first()
            .and_then(|kind| {
                super::clients::category_for(kind).map(|category| Connection::Extraction {
                    media: kind.clone(),
                    category: category.value,
                })
            })
            .ok_or(Unmade::Files),
        _ => Err(Unmade::Unpaired),
    }
}

/// Every asker paired with each service that fills what it asked for, for the asks
/// answered here: the upstream it fronts, where that service is an adapter, since the
/// asker is told about a service in that service's own API. An asker an adapter of its
/// plugin fronts is paired by the contract that adapter speaks, and the adapter is asked.
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
        let answering = fillers.fronting(asker).unwrap_or(asker);
        let Some(contract) = asking(answering, &ask.capability) else {
            continue;
        };
        for filler in &ask.fillers {
            let filler = fillers.fronted_by(filler).unwrap_or(filler);
            found.push(Pairing {
                asker,
                ask,
                filler,
                made: made(fillers, (asker, answering, contract), ask, filler),
            });
        }
    }
    found
}

/// What one asker, asked through `answering` and paired by `contract`, and one filler come
/// to.
fn made<'a>(
    fillers: &Fillers,
    (asker, answering, contract): (&'a Filler, &'a Filler, &'static str),
    ask: &Ask,
    filler: &'a Filler,
) -> Result<(Connection, &'a Address, Cleared<'a>), Unmade> {
    if !fillers.hands(filler, answering) {
        return Err(Unmade::Asked);
    }
    let speaks = filler.talks_in().ok_or(Unmade::NoNative)?;
    let at = filler.address.as_ref().ok_or(Unmade::NoPort)?;
    let made = connection(
        contract,
        &ask.capability,
        speaks,
        &filler.media_types,
        &asker.media_types,
    )?;
    if made.hands_over_the_askers_key()
        && !crate::wiring::crosses(answering.holder(), filler.holder())
    {
        return Err(Unmade::Withheld);
    }
    Ok((
        made,
        at,
        Cleared {
            named: asker,
            answering,
        },
    ))
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

/// Whether the filler asks for something the asker fills, itself or through the adapter
/// in front of it.
fn from_its_end(fillers: &Fillers, pairing: &Pairing<'_>) -> bool {
    fillers.asks().iter().any(|ask| {
        ask.by == pairing.filler.id
            && ask
                .fillers
                .iter()
                .any(|one| fillers.fronted_by(one).unwrap_or(one).id == pairing.asker.id)
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
        Unmade::NoNative => format!(
            "{fills} and names no API of its own that lemonfiber could tell {asker} about it in"
        ),
        Unmade::NoPort => {
            format!("{fills} and does not say which port it answers on inside the stack's network")
        }
        Unmade::Unpaired => format!(
            "{fills} and nothing in lemonfiber tells {asker} about a service answering in \
             {filler}'s API"
        ),
        Unmade::Synced => format!(
            "{fills} and syncs the media {asker} files to it itself, so {filler} is told about \
             {asker} rather than {asker} about {filler}"
        ),
        Unmade::Files if pairing.filler.media_types.is_empty() => format!(
            "{fills} and names no media it files, so lemonfiber cannot say what to hand {asker}"
        ),
        Unmade::Files => format!(
            "{fills} and files {}, which lemonfiber does not hand {asker}",
            pairing.filler.media_types.join(" and ")
        ),
        Unmade::Withheld if matches!(pairing.filler.holder(), Holder::ThirdParty(_)) => format!(
            "{fills} and is a third-party plugin's service; {asker} hands every service it \
             registers its own API key, which opens every indexer it holds, so lemonfiber does \
             not register a third-party plugin's service in it"
        ),
        Unmade::Withheld => format!(
            "{fills} and {asker} is a third-party plugin's service; {asker} hands every service \
             it registers its own API key, and a third-party plugin's credential is handed only \
             to its own plugin's services"
        ),
        Unmade::Asked if matches!(pairing.asker.holder(), Holder::ThirdParty(_)) => format!(
            "{fills} and {asker} is a third-party plugin's service, which is handed another \
             service's credential only where the operator granted it in an install or update"
        ),
        Unmade::Asked => format!(
            "{fills} and is a third-party plugin's service, whose credential is handed only to \
             the stack's own services and to its own plugin's"
        ),
    }
}

#[cfg(test)]
mod tests;
