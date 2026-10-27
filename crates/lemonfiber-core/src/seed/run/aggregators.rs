//! Telling a curator that pulls its indexers where they come from.
//!
//! Most curators are registered into by the aggregator itself. One whose media the
//! aggregator's application sync does not serve keeps its own list of aggregators and
//! pulls from them, so the connection is made from the other end: it is told where one
//! is, in the aggregator's native API, and handed a key to read it with. It is asked
//! over `indexer.pull`, and one that answers it cannot pull from that aggregator is left
//! unpaired.

use lemonfiber_contract::capabilities::indexer::pull;

use super::connecting::{pairings, Connection};
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::ports::service::{AggregatorAt, Failure, Protocol, PullsIndexers as _};
use crate::wiring::{Filler, Fillers};

/// What this connection is called where it is reported.
fn connection(asker: &str) -> String {
    format!("Indexers into {asker}")
}

/// Tell each curator that pulls its indexers about the aggregator its ask for an indexer
/// settles on, the stack's or a plugin's.
///
/// Which service that is comes from what the stack says the curator asks for, not from a
/// name written here, and it is reached where it says it listens on the stack's network
/// with the key lemonfiber minted for it. Only a curator the gate lets the aggregator's key
/// reach is told, since it is handed that key: a third-party plugin's service is handed
/// no credential that is not its own. A pair nothing here connects is reported by the
/// table, never dropped.
///
/// Nothing for an aggregator lemonfiber holds no minted key for.
pub(super) async fn seed_aggregators(ctx: &Ctx, fillers: &Fillers) -> Vec<crate::seed::Wiring> {
    let mut wirings = Vec::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::Aggregator, at, asker)) = pairing.made else {
            continue;
        };
        let (Some(puller), Some(native)) = (
            puller(ctx, asker.answering()).await,
            pairing.filler.talks_in(),
        ) else {
            continue;
        };
        let connection = connection(&asker.name);
        let Some(key) = super::keys::minted_key(ctx, pairing.filler) else {
            continue;
        };
        let aggregator = AggregatorAt {
            name: pairing.filler.name.clone(),
            url: at.url(),
            protocol: Protocol(native.name().to_owned()),
            key,
        };
        wirings.push(crate::seed::Wiring::settled(
            connection,
            told(&puller, &asker.name, &aggregator, ctx.dry_run).await,
        ));
    }
    wirings
}

/// The curator as this pass tells it, over `indexer.pull`: nothing where it does not
/// speak the contract or cannot be asked over it.
async fn puller(ctx: &Ctx, asker: &Filler) -> Option<pull::Adapter> {
    match spoken(ctx, asker, pull::CAPABILITY, pull::MAJOR).await {
        Spoken::Over(adapter) => Some(pull::Adapter(adapter)),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// Point the service at the aggregator, leaving it alone where it already is.
///
/// **An entry without a key counts as absent.** The service takes a registration whose
/// key it did not understand and answers success, so one already there is only already
/// wired if it holds a key — otherwise it is the failure this exists to prevent,
/// wearing the shape of a connection that was made.
///
/// **One that cannot pull from it is left unpaired.** A curator answering `unsupported`
/// reads no aggregator of that native API, which is a pair nothing here connects rather
/// than a fault.
async fn told(
    puller: &pull::Adapter,
    asker: &str,
    aggregator: &AggregatorAt,
    rehearsing: bool,
) -> crate::seed::State {
    let held = match puller.aggregators().await {
        Ok(held) => held,
        Err(failure) => return unreached(asker, aggregator, &failure),
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
    if let Err(failure) = puller.add_aggregator(aggregator).await {
        return unreached(asker, aggregator, &failure);
    }
    crate::seed::State::Wired
}

/// A curator that would not take the aggregator, in its own words: unpaired where it
/// cannot pull from one of that native API, and otherwise as any other failure.
fn unreached(asker: &str, aggregator: &AggregatorAt, failure: &Failure) -> crate::seed::State {
    match failure {
        Failure::Unsupported { detail, .. } => crate::seed::State::Unmatched {
            reason: format!(
                "{asker} cannot pull its indexers from {}, which it would read in the {} API: \
                 {detail}",
                aggregator.name, aggregator.protocol.0
            ),
        },
        _ => crate::seed::unreached(failure),
    }
}
