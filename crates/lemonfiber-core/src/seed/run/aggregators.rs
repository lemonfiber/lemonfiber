//! Telling a curator that pulls its indexers where they come from.
//!
//! Most curators are registered into by the aggregator itself. One whose media the
//! aggregator's application sync does not serve keeps its own list of aggregators and
//! pulls from them, so the connection is made from the other end: it is told where one
//! is, in the aggregator's native API, and handed a key to read it with. So is one whose
//! media an app sync would serve, where the aggregator answers that it has none. It is
//! asked over `indexer.pull`, and one that answers it cannot pull from that aggregator is
//! left unpaired.

use lemonfiber_contract::capabilities::indexer::{pull, search};

use super::connecting::{from_its_end, pairings, Connection, Pairing};
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::ports::service::{AggregatorAt, AppSync as _, Failure, Protocol, PullsIndexers as _};
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
/// A curator whose media an app sync serves is told only where the aggregator answers
/// that it has no app sync; otherwise it is left to that sync, and reported where the
/// aggregator does not ask for it. One whose aggregator declares no key of its own is
/// reported unpaired, and one whose key lemonfiber does not hold yet is skipped.
pub(super) async fn seed_aggregators(ctx: &Ctx, fillers: &Fillers) -> Vec<crate::seed::Wiring> {
    let mut wirings = Vec::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::Aggregator { synced }, at, asker)) = pairing.made else {
            continue;
        };
        let (Some(puller), Some(native)) = (
            puller(ctx, asker.answering()).await,
            pairing.filler.talks_in(),
        ) else {
            continue;
        };
        let connection = connection(&asker.name);
        if synced {
            if let Some(left) = left_to_app_sync(ctx, fillers, &pairing).await {
                wirings.extend(left.map(|state| crate::seed::Wiring::settled(connection, state)));
                continue;
            }
        }
        if super::keys::keyed(pairing.filler).is_none() {
            wirings.push(crate::seed::Wiring::settled(
                connection,
                crate::seed::State::Unmatched {
                    reason: unkeyed(&pairing),
                },
            ));
            continue;
        }
        let Some(key) = super::keys::minted_key(ctx, pairing.filler) else {
            wirings.push(super::applications::skipped(
                connection,
                &pairing.filler.name,
            ));
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

/// What a curator whose media an app sync serves is left as, where the aggregator's app
/// sync is what reaches it: nothing to report where the aggregator asks for it, said as
/// unpaired where it does not, and the failure where the aggregator could not be asked.
/// Nothing where the aggregator answers that it has no app sync, so the curator is told
/// about it instead.
async fn left_to_app_sync(
    ctx: &Ctx,
    fillers: &Fillers,
    pairing: &Pairing<'_>,
) -> Option<Option<crate::seed::State>> {
    let aggregator = fillers.fronting(pairing.filler).unwrap_or(pairing.filler);
    let answered = match spoken(ctx, aggregator, search::CAPABILITY, search::MAJOR).await {
        Spoken::Over(adapter) => search::Adapter(adapter).applications().await.map(|_| ()),
        Spoken::Unanswered | Spoken::Not => Ok(()),
    };
    match answered {
        Err(Failure::Unsupported { .. }) => None,
        Err(failure) => Some(Some(crate::seed::unreached(&failure))),
        Ok(()) if from_its_end(fillers, pairing) => Some(None),
        Ok(()) => Some(Some(crate::seed::State::Unmatched {
            reason: synced(pairing),
        })),
    }
}

/// What the aggregator and the curator are, said as the table says a pair: what fills
/// what the curator asks for.
fn fills(pairing: &Pairing<'_>) -> String {
    format!(
        "{} fills {}, which {} asks for,",
        pairing.filler.name, pairing.ask.capability, pairing.asker.name
    )
}

/// Why a curator whose media an app sync serves is not told about the aggregator.
fn synced(pairing: &Pairing<'_>) -> String {
    let (filler, asker) = (&pairing.filler.name, &pairing.asker.name);
    format!(
        "{} and syncs the media {asker} files to it itself, so {filler} is told about {asker} \
         rather than {asker} about {filler}",
        fills(pairing)
    )
}

/// Why a curator is not told about an aggregator that declares no key of its own.
fn unkeyed(pairing: &Pairing<'_>) -> String {
    let (filler, asker) = (&pairing.filler.name, &pairing.asker.name);
    format!(
        "{} and declares no key lemonfiber mints for it, which {asker} would read {filler} with",
        fills(pairing)
    )
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
        return pulled_again(puller, rehearsing).await;
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

/// A curator already reading from the aggregator, asked to read its indexers from it
/// again, so one added to the aggregator since reaches it.
///
/// Not in a rehearsal, which asks nothing of a service. A curator that cannot be asked to
/// keeps the pair as it was.
async fn pulled_again(puller: &pull::Adapter, rehearsing: bool) -> crate::seed::State {
    if rehearsing {
        return crate::seed::State::AlreadyWired;
    }
    match puller.pull().await {
        Ok(()) | Err(Failure::Unsupported { .. }) => crate::seed::State::AlreadyWired,
        Err(failure) => crate::seed::unreached(&failure),
    }
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
