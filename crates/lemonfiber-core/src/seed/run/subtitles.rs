//! Telling the subtitle finder which \*arrs to watch.
//!
//! The last of the connections that runs *into* a service rather than out of one:
//! the finder does not discover the \*arrs, and until it is told it has nothing to
//! look at. A household then gets subtitles for nothing, which is indistinguishable
//! from releases that happen to have none — the failure this whole feature exists
//! to prevent, in its quietest form.
//!
//! Each \*arr is told about on its own. The service takes a partial write, so one
//! that is not running is skipped and completed on a later pass rather than holding
//! up the other.

use super::connecting::{pairings, Connection, FILM, TELEVISION};
use super::Ctx;
use crate::ports::filesystem::Beneath;
use crate::ports::service::{Subtitled, Subtitles as _, Watched};
use crate::wiring::{Address, Filler, Fillers};

/// Which \*arr this is to the subtitle finder, or nothing where it files media that
/// carries no subtitles.
///
/// Music and books are not an omission: there is nothing to subtitle, so the finder
/// has no setting for them at all.
pub(super) fn subtitled(media_types: &[String]) -> Option<Subtitled> {
    if media_types.iter().any(|kind| kind == TELEVISION) {
        return Some(Subtitled::Sonarr);
    }
    if media_types.iter().any(|kind| kind == FILM) {
        return Some(Subtitled::Radarr);
    }
    None
}

/// One subtitle finder, and every curator it is told about with where it reaches each.
struct Watching<'a> {
    /// The finder that asks.
    asker: &'a Filler,
    /// Each curator, as the finder files it, and where it reaches it.
    curators: Vec<(&'a Filler, Subtitled, &'a Address)>,
}

/// Every subtitle finder and the curators lemonfiber tells it about.
fn watching(fillers: &Fillers) -> Vec<Watching<'_>> {
    let mut found: Vec<Watching<'_>> = Vec::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::Subtitles(which), at)) = pairing.made else {
            continue;
        };
        match found
            .iter_mut()
            .find(|one| one.asker.id == pairing.asker.id)
        {
            Some(one) => one.curators.push((pairing.filler, which, at)),
            None => found.push(Watching {
                asker: pairing.asker,
                curators: vec![(pairing.filler, which, at)],
            }),
        }
    }
    found
}

/// The finder as a client holding the key it wrote for itself, or nothing where this
/// machine cannot reach it or it has not written one yet — a service still starting
/// rather than a fault, so a later run completes it. A finder is one of the stack's own
/// services, whose credential file is never confined, so nothing here is refused.
async fn finder(ctx: &Ctx, asker: &Filler) -> Option<crate::bazarr::Bazarr> {
    let published = asker.published?;
    let key = crate::bazarr::api_key(
        &crate::app::targets::credential_file(ctx, asker)
            .await
            .text()?,
    )?;
    Some(crate::bazarr::Bazarr::new(
        ctx.seams.http.clone(),
        crate::app::targets::loopback(published),
        &asker.id,
        key,
    ))
}

/// What a curator's watch in a subtitle finder is called where it is reported.
fn for_subtitles(curator: &str) -> String {
    format!("{curator} watched for subtitles")
}

/// Tell each subtitle finder about every curator whose media has subtitles.
///
/// Nothing to do where the stack has no subtitle finder, or where its key has not
/// been written yet — the second is a service still starting rather than a fault, so
/// it is skipped and a later run completes it.
pub(super) async fn seed_subtitles(ctx: &Ctx, fillers: &Fillers) -> Vec<crate::seed::Wiring> {
    let mut wirings = Vec::new();
    for watching in watching(fillers) {
        let Some(finder) = finder(ctx, watching.asker).await else {
            continue;
        };
        for (curator, which, at) in &watching.curators {
            let connection = for_subtitles(&curator.name);
            let api_key = match super::arrs::servarr_key(ctx, curator).await {
                Beneath::Read(key) => key,
                Beneath::Absent => {
                    wirings.push(super::skipped(connection, &curator.name));
                    continue;
                }
                Beneath::Escaped => {
                    wirings.push(super::arrs::refused(connection, curator));
                    continue;
                }
            };
            let watched = Watched {
                which: *which,
                host: at.host.clone(),
                port: at.port,
                api_key,
            };
            wirings.push(crate::seed::Wiring::settled(
                connection,
                watch(&finder, &watched, ctx.dry_run).await,
            ));
        }
    }
    wirings
}

/// Point each finder watching the curator `arr` at it with the key it answers to now,
/// whatever the finder holds: what replacing that curator's key owes it. The finder
/// shows whether it holds a key and never which, so there is nothing to read first.
///
/// Answers with each finder's name and how the write came out — none where no finder
/// watches `arr`, and none from one where a key is not written yet, since there is
/// nothing to hold to it until there is.
pub(crate) async fn rewatch(
    ctx: &Ctx,
    fillers: &Fillers,
    arr: &str,
) -> Vec<(String, crate::seed::State)> {
    let mut found = Vec::new();
    for watching in watching(fillers) {
        let Some((curator, which, at)) = watching
            .curators
            .iter()
            .find(|(curator, _, _)| curator.id == arr)
        else {
            continue;
        };
        let Some(finder) = finder(ctx, watching.asker).await else {
            continue;
        };
        let api_key = match super::arrs::servarr_key(ctx, curator).await {
            Beneath::Read(key) => key,
            Beneath::Absent => continue,
            Beneath::Escaped => {
                found.push((watching.asker.name.clone(), super::arrs::refusal(curator)));
                continue;
            }
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

/// Point the finder at one \*arr, leaving it alone where it already is.
///
/// Read first, because the operator may have set this themselves or a previous run
/// may have done it: writing regardless would be a second write that changes nothing
/// and reports as though it had.
async fn watch(
    finder: &crate::bazarr::Bazarr,
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
    // watching this \*arr is left alone on a real run, so a rehearsal of that is the
    // run. What is reported is the address a real one would point it at, and what it
    // holds now where it holds anything at all — including whether it holds a key for
    // it, because a finder pointed at the right \*arr with no key is exactly the case
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

#[cfg(test)]
mod tests;
