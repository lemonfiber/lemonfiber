//! Telling the subtitle finder which curators to watch.
//!
//! The last of the connections that runs *into* a service rather than out of one:
//! the finder does not discover the curators, and until it is told it has nothing to
//! look at. A household then gets subtitles for nothing, which is indistinguishable
//! from releases that happen to have none — the failure this whole feature exists
//! to prevent, in its quietest form.
//!
//! Each curator is told about on its own. The service takes a partial write, so one
//! that is not running is skipped and completed on a later pass rather than holding
//! up the other.

use lemonfiber_contract::capabilities::subtitles::fetch;

use super::connecting::{pairings, Cleared, Connection};
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::ports::media::Kind;
use crate::ports::service::Watched;
use crate::wiring::{Address, Filler, Fillers};

/// One subtitle finder, and every curator it is told about with where it reaches each.
struct Watching<'a> {
    /// The finder that asks.
    asker: Cleared<'a>,
    /// Each curator, as the finder files it, and where it reaches it.
    curators: Vec<(&'a Filler, Kind, &'a Address)>,
}

/// Every subtitle finder and the curators lemonfiber tells it about, each curator once
/// however many of the finder's asks it fills.
fn watching(fillers: &Fillers) -> Vec<Watching<'_>> {
    let mut found: Vec<Watching<'_>> = Vec::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::Subtitles(which), at, asker)) = pairing.made else {
            continue;
        };
        match found.iter_mut().find(|one| one.asker.id == asker.id) {
            Some(one)
                if one
                    .curators
                    .iter()
                    .any(|(told, _, _)| told.id == pairing.filler.id) => {}
            Some(one) => one.curators.push((pairing.filler, which, at)),
            None => found.push(Watching {
                asker,
                curators: vec![(pairing.filler, which, at)],
            }),
        }
    }
    found
}

/// The finder as the core asks it: over `subtitles.fetch` where it speaks the contract.
/// Nothing where it speaks it and cannot be asked over it, and nothing for a finder that
/// speaks no contract, which the core has no other way to ask.
async fn finder(ctx: &Ctx, asker: Cleared<'_>) -> Option<Box<dyn fetch::Fills>> {
    match spoken(ctx, asker.answering(), fetch::CAPABILITY, fetch::MAJOR).await {
        Spoken::Over(adapter) => Some(Box::new(fetch::Adapter(adapter))),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// What a curator's watch in a subtitle finder is called where it is reported.
fn for_subtitles(curator: &str) -> String {
    format!("{curator} watched for subtitles")
}

/// Tell each subtitle finder about every curator whose media has subtitles.
///
/// Nothing to do where the stack has no subtitle finder; a curator lemonfiber holds no
/// minted key for is skipped.
pub(super) async fn seed_subtitles(ctx: &Ctx, fillers: &Fillers) -> Vec<crate::seed::Wiring> {
    let mut wirings = Vec::new();
    for watching in watching(fillers) {
        let Some(finder) = finder(ctx, watching.asker).await else {
            continue;
        };
        for (curator, which, at) in &watching.curators {
            let connection = for_subtitles(&curator.name);
            let Some(api_key) = super::keys::minted_key(ctx, curator) else {
                wirings.push(super::skipped(connection, &curator.name));
                continue;
            };
            let watched = Watched {
                which: *which,
                host: at.host.clone(),
                port: at.port,
                api_key,
            };
            wirings.push(crate::seed::Wiring::settled(
                connection,
                watch(finder.as_ref(), &watched, ctx.dry_run).await,
            ));
        }
    }
    wirings
}

/// Point each finder watching the curator `curator_id` at it with the key it answers to
/// now, whatever the finder holds: what replacing that curator's key owes it. The finder
/// shows whether it holds a key and never which, so there is nothing to read first.
///
/// Answers with each finder's name and how the write came out — none where no finder
/// watches `curator_id`, and none where lemonfiber holds no minted key for it, since there
/// is nothing to hold to it.
pub(crate) async fn rewatch(
    ctx: &Ctx,
    fillers: &Fillers,
    curator_id: &str,
) -> Vec<(String, crate::seed::State)> {
    let mut found = Vec::new();
    for watching in watching(fillers) {
        let Some((curator, which, at)) = watching
            .curators
            .iter()
            .find(|(curator, _, _)| curator.id == curator_id)
        else {
            continue;
        };
        let Some(finder) = finder(ctx, watching.asker).await else {
            continue;
        };
        let Some(api_key) = super::keys::minted_key(ctx, curator) else {
            continue;
        };
        let watched = Watched {
            which: *which,
            host: at.host.clone(),
            port: at.port,
            api_key,
        };
        let state = match finder.watch(&watched).await {
            Ok(()) => crate::seed::State::Wired,
            Err(failure) => unreached(&failure),
        };
        found.push((watching.asker.name.clone(), state));
    }
    found
}

/// Point the finder at one curator, leaving it alone where it already is.
///
/// Read first, because the operator may have set this themselves or a previous run
/// may have done it: writing regardless would be a second write that changes nothing
/// and reports as though it had.
async fn watch(
    finder: &dyn fetch::Fills,
    watched: &Watched,
    rehearsing: bool,
) -> crate::seed::State {
    let held = match finder.watching(watched.which).await {
        Ok(held) => held,
        Err(failure) => return unreached(&failure),
    };
    if held.enabled && held.host == watched.host && held.port == watched.port && held.keyed {
        return crate::seed::State::AlreadyWired;
    }
    // Below the read, for the reason every other gate here is: a finder already
    // watching this curator is left alone on a real run, so a rehearsal of that is the
    // run. What is reported is the address a real one would point it at, and what it
    // holds now where it holds anything at all — including whether it holds a key for
    // it, because a finder pointed at the right curator with no key is exactly the case
    // this connection exists to fix, and the two addresses on their own would read as
    // a change to nothing.
    if rehearsing {
        let keyed = if held.keyed { "" } else { ", with no key" };
        return crate::seed::State::WouldWire {
            yours: held
                .enabled
                .then(|| format!("{}:{}{keyed}", held.host, held.port)),
            ours: Some(format!(
                "{}:{}, with lemonfiber's key",
                watched.host, watched.port
            )),
        };
    }
    if let Err(failure) = finder.watch(watched).await {
        return unreached(&failure);
    }
    crate::seed::State::Wired
}

/// A service that would not answer, in its own words.
fn unreached(failure: &crate::ports::service::Failure) -> crate::seed::State {
    crate::seed::State::Failed {
        detail: failure.to_string(),
    }
}
