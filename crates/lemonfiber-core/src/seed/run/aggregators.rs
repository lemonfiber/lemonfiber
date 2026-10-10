//! Telling the book \*arr where its indexers come from.
//!
//! Every other \*arr is registered into by the aggregator itself. This one the
//! aggregator cannot reach, so the connection is made from the other end: the service
//! keeps its own list of aggregators and pulls from them, and what has to happen is
//! that it is told where one is and handed a key to read it with.
//!
//! Its own key is minted here and handed to the service through its environment, which
//! it adopts in place of generating one — otherwise the key would live only in a
//! database, and nothing outside the service could present it.

use super::connecting::{pairings, Connection};
use super::Ctx;
use crate::ports::filesystem::Beneath;
use crate::ports::service::{Aggregator, Aggregators as _};
use crate::wiring::{Filler, Fillers};

/// What this connection is called where it is reported.
fn connection(asker: &str) -> String {
    format!("Indexers into {asker}")
}

/// Tell each book \*arr about the aggregator its ask for an indexer settles on, the
/// stack's or a plugin's.
///
/// Which service that is comes from what the stack says the book \*arr asks for, not
/// from a name written here, and it is reached where it says it listens on the stack's
/// network with the key it wrote for itself. Only a book \*arr the gate lets the
/// aggregator's key reach is told, since it is handed that key: a third-party plugin's
/// service is handed no credential that is not its own. A pair nothing here connects is
/// reported by the table, never dropped.
///
/// Nothing for a book \*arr with no key yet — the key is minted on the run that first
/// reaches it, and a service started before that is completed by a later run rather
/// than failed — or for an aggregator that has not written its own key yet.
pub(super) async fn seed_aggregators(ctx: &Ctx, fillers: &Fillers) -> Vec<crate::seed::Wiring> {
    let mut wirings = Vec::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::Aggregator, at, asker)) = pairing.made else {
            continue;
        };
        let Some(client) = reader(ctx, fillers, &asker) else {
            continue;
        };
        let connection = connection(&asker.name);
        let key = match super::curating::servarr_key(ctx, pairing.filler).await {
            Beneath::Read(key) => key,
            Beneath::Absent => continue,
            Beneath::Escaped => {
                wirings.push(super::curating::refused(connection, pairing.filler));
                continue;
            }
        };
        let aggregator = Aggregator {
            name: pairing.filler.name.clone(),
            url: at.url(),
            key,
        };
        wirings.push(crate::seed::Wiring::settled(
            connection,
            told(&client, &aggregator, ctx.dry_run).await,
        ));
    }
    wirings
}

/// The book \*arr as a client holding the key lemonfiber minted for it, under the setting
/// kept for it, where it publishes a port and the key is recorded.
///
/// The key is minted where the services are started, before this one has ever run, and
/// the service adopts it from its environment then — so the recorded value is what both
/// sides hold.
fn reader(ctx: &Ctx, fillers: &Fillers, asker: &Filler) -> Option<crate::bindery::Bindery> {
    let port = asker.published?;
    let setting = fillers.setting(asker, crate::config::API_KEY_SUFFIX)?;
    let key = crate::app::targets::recorded_secret(ctx, &setting)?;
    Some(crate::bindery::Bindery::new(
        ctx.seams.http.clone(),
        crate::app::targets::loopback(port),
        &asker.id,
        key,
    ))
}

/// Point the service at the aggregator, leaving it alone where it already is.
///
/// **An entry without a key counts as absent.** The service takes a registration whose
/// key it did not understand and answers success, so one already there is only already
/// wired if it holds a key — otherwise it is the failure this exists to prevent,
/// wearing the shape of a connection that was made.
async fn told(
    client: &crate::bindery::Bindery,
    aggregator: &Aggregator,
    rehearsing: bool,
) -> crate::seed::State {
    let held = match client.aggregators().await {
        Ok(held) => held,
        Err(failure) => return unreached(&failure),
    };
    if held
        .iter()
        .any(|known| known.url == aggregator.url && known.keyed)
    {
        return crate::seed::State::AlreadyWired;
    }
    // Below the read and the already-there check, because both are true of a real run
    // too: what a rehearsal leaves out is the registration, and nothing above it.
    if rehearsing {
        return crate::seed::State::WouldWire {
            yours: None,
            ours: Some(aggregator.url.clone()),
        };
    }
    if let Err(failure) = client.add_aggregator(aggregator).await {
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
