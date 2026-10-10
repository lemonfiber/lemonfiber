//! Pushing the services at the indexer that searches for them.
//!
//! The indexer needs to know what to search on behalf of, which is the one connection
//! that runs from the indexer outward rather than into it.

use lemonfiber_contract::capabilities::indexer::search;

use super::connecting::{pairings, Cleared, Connection};
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::ports::filesystem::Beneath;
use crate::ports::service::{AppSync, Application};
use crate::wiring::{Filler, Fillers};

/// One indexer and the curators it is told about: each one's application, or the
/// wiring that says why it is not told yet.
struct Syncing<'a> {
    /// The indexer that asks.
    asker: Cleared<'a>,
    /// Each curator, with the application it comes to.
    curators: Vec<(&'a Filler, crate::ports::service::ApplicationKind, String)>,
}

/// Every indexer and the curators lemonfiber registers into it, each with the address
/// the indexer reaches it on.
fn syncing(fillers: &Fillers) -> Vec<Syncing<'_>> {
    let mut found: Vec<Syncing<'_>> = Vec::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::Application(kind), at, asker)) = pairing.made else {
            continue;
        };
        let reached = at.url();
        match found.iter_mut().find(|one| one.asker.id == asker.id) {
            Some(one) => one.curators.push((pairing.filler, kind, reached)),
            None => found.push(Syncing {
                asker,
                curators: vec![(pairing.filler, kind, reached)],
            }),
        }
    }
    found
}

/// How the indexer is asked.
enum Asked {
    /// By this client.
    By(Box<dyn AppSync>),
    /// Not at all: it cannot be reached, or speaks the contract and cannot be asked over it.
    Nobody,
    /// Not yet: it is the bundled indexer and its own key is not written yet.
    Unkeyed,
}

/// The indexer as the core asks it: over `indexer.search` where it speaks the contract,
/// otherwise as the bundled indexer holding its own key.
async fn indexer(ctx: &Ctx, asker: &Cleared<'_>) -> Asked {
    match spoken(ctx, asker, search::CAPABILITY, search::MAJOR).await {
        Spoken::Over(adapter) => return Asked::By(Box::new(search::Adapter(adapter))),
        Spoken::Unanswered => return Asked::Nobody,
        Spoken::Not => {}
    }
    let Some(published) = asker.published else {
        return Asked::Nobody;
    };
    let Beneath::Read(key) = super::curating::servarr_key(ctx, asker).await else {
        return Asked::Unkeyed;
    };
    Asked::By(Box::new(crate::prowlarr::Prowlarr::new(
        ctx.seams.http.clone(),
        crate::app::targets::loopback(published),
        key,
        &asker.id,
    )))
}

/// Register each curator the indexer asks for as an application in it, so the indexer
/// pushes it indexers.
///
/// Two keys gate a write, both read from configuration and never asked for: the
/// indexer's own — without it the indexer is still starting, so every application is
/// skipped for a re-run — and each curator's, which is what lets the indexer write into
/// it; a curator that has not written its key yet is skipped on its own while the
/// others proceed.
pub(super) async fn seed_applications(ctx: &Ctx, fillers: &Fillers) -> Vec<crate::seed::Wiring> {
    let mut wirings = Vec::new();
    for syncing in syncing(fillers) {
        wirings.extend(sync(ctx, &syncing, None).await);
    }
    wirings
}

/// What one indexer comes to: each curator's application wired, or skipped where a key
/// it needs is not written yet. Only `only` where one is named.
async fn sync(ctx: &Ctx, syncing: &Syncing<'_>, only: Option<&str>) -> Vec<crate::seed::Wiring> {
    let asker = &*syncing.asker;
    let curators: Vec<_> = syncing
        .curators
        .iter()
        .filter(|(curator, _, _)| only.is_none_or(|id| curator.id == id))
        .collect();
    let Some(back) = asker.address.as_ref() else {
        return Vec::new();
    };
    if curators.is_empty() {
        return Vec::new();
    }
    let client = match indexer(ctx, &syncing.asker).await {
        Asked::By(client) => client,
        Asked::Nobody => return Vec::new(),
        Asked::Unkeyed => {
            return curators
                .iter()
                .map(|(curator, _, _)| skipped(synced(&curator.name, &asker.name), &asker.name))
                .collect()
        }
    };
    let mut wanted = Vec::new();
    let mut passed = Vec::new();
    for (curator, kind, reached) in curators {
        // A curator the gate let the indexer's key reach, read from its own file: one not
        // written yet, or in a file it may not be read from, is passed over this run.
        let Beneath::Read(key) = super::curating::servarr_key(ctx, curator).await else {
            passed.push(skipped(synced(&curator.name, &asker.name), &curator.name));
            continue;
        };
        wanted.push(Application {
            name: curator.name.clone(),
            kind: *kind,
            indexer_url: back.url(),
            base_url: reached.clone(),
            api_key: key,
        });
    }
    // The journal seed records each write into is not persisted: seeding is
    // idempotent, so a partial run is recovered by running it again, not reversed
    // — see the seed module doc. The record is groundwork for a future service-side
    // undo the current reversal cannot do.
    let mut journal = crate::journal::Journal::new();
    let mut wirings = crate::seed::wire_applications(
        client.as_ref(),
        &asker.name,
        &wanted,
        &mut journal,
        &ctx.stamp(),
        ctx.dry_run,
    )
    .await;
    wirings.extend(passed);
    wirings
}

/// Hold each indexer's application for the curator `arr` to the key it answers to now:
/// what replacing that curator's key owes the indexer.
///
/// Answers with the indexer's name and how the application came out, for each indexer
/// that registers it — none where nothing does. One where either key is not written yet
/// is passed over: until both are there is no application to hold to anything.
pub(crate) async fn resync_application(
    ctx: &Ctx,
    fillers: &Fillers,
    arr: &str,
) -> Vec<(String, crate::seed::State)> {
    let mut found = Vec::new();
    for syncing in syncing(fillers) {
        for wiring in sync(ctx, &syncing, Some(arr)).await {
            if !matches!(wiring.state, crate::seed::State::Skipped { .. }) {
                found.push((syncing.asker.name.clone(), wiring.state));
            }
        }
    }
    found
}

/// The application kind for an \*arr's media, read from the first media type it
/// declares, or nothing where that is not one an indexer's app sync covers — the same
/// by-media mapping the download clients' categories use, so the two stay in step.
pub(super) fn application_kind(
    media_types: &[String],
) -> Option<crate::ports::service::ApplicationKind> {
    media_types
        .first()
        .and_then(|media| crate::ports::service::ApplicationKind::for_media_type(media))
}

/// A `Wiring` skipped because the service has not written the key it needs yet — a
/// re-run completes it. The reason is worded once here so a re-run's report reads
/// the same across root folders, download clients and application sync; the
/// `connection` names the specific edge being skipped.
pub(super) fn skipped(connection: String, service: &str) -> crate::seed::Wiring {
    crate::seed::Wiring::settled(
        connection,
        crate::seed::State::Skipped {
            reason: format!("{service} has not written its API key yet; a later run completes it"),
        },
    )
}

/// What a curator's application in an indexer is called where it is reported.
fn synced(curator: &str, indexer: &str) -> String {
    format!("{curator} indexer sync via {indexer}")
}
