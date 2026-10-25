//! Telling a curator that pulls its indexers where they come from.
//!
//! Most curators are registered into by the aggregator itself. One whose media the
//! aggregator's application sync does not serve keeps its own list of aggregators and
//! pulls from them, so the connection is made from the other end: it is told where one
//! is, in the aggregator's native API, and handed a key to read it with. It is asked
//! over `indexer.pull` where it speaks the contract, and one that answers it cannot
//! pull from that aggregator is left unpaired.
//!
//! Its own key is minted here and handed to the service through its environment, which
//! it adopts in place of generating one — otherwise the key would live only in a
//! database, and nothing outside the service could present it.

use lemonfiber_contract::capabilities::indexer::pull;

use super::connecting::{pairings, Connection};
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::ports::filesystem::Beneath;
use crate::ports::service::{
    Aggregator, AggregatorAt, Aggregators as _, Failure, KnownAggregator, Protocol,
    PullsIndexers as _,
};
use crate::wiring::{Filler, Fillers};

/// A curator that pulls its indexers, as this pass tells it about an aggregator.
enum Puller {
    /// Over `indexer.pull`, through its plugin's adapter.
    Over(pull::Adapter),
    /// The stack's own book curator, through the adapter this build holds.
    Bundled(crate::bindery::Bindery),
}

impl Puller {
    /// The aggregators it already reads from.
    async fn aggregators(&self) -> Result<Vec<KnownAggregator>, Failure> {
        match self {
            Self::Over(adapter) => adapter.aggregators().await,
            Self::Bundled(client) => client.aggregators().await,
        }
    }

    /// Read its indexers from `aggregator` as well.
    async fn add(&self, aggregator: &AggregatorAt) -> Result<(), Failure> {
        match self {
            Self::Over(adapter) => adapter.add_aggregator(aggregator).await,
            Self::Bundled(client) => {
                client
                    .add_aggregator(&Aggregator {
                        name: aggregator.name.clone(),
                        url: aggregator.url.clone(),
                        key: aggregator.key.clone(),
                    })
                    .await
            }
        }
    }
}

/// What this connection is called where it is reported.
fn connection(asker: &str) -> String {
    format!("Indexers into {asker}")
}

/// Tell each curator that pulls its indexers about the aggregator its ask for an indexer
/// settles on, the stack's or a plugin's.
///
/// Which service that is comes from what the stack says the book curator asks for, not
/// from a name written here, and it is reached where it says it listens on the stack's
/// network with the key it wrote for itself. Only a book curator the gate lets the
/// aggregator's key reach is told, since it is handed that key: a third-party plugin's
/// service is handed no credential that is not its own. A pair nothing here connects is
/// reported by the table, never dropped.
///
/// Nothing for a book curator with no key yet — the key is minted on the run that first
/// reaches it, and a service started before that is completed by a later run rather
/// than failed — or for an aggregator that has not written its own key yet.
pub(super) async fn seed_aggregators(ctx: &Ctx, fillers: &Fillers) -> Vec<crate::seed::Wiring> {
    let mut wirings = Vec::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::Aggregator, at, asker)) = pairing.made else {
            continue;
        };
        let Some(puller) = puller(ctx, fillers, asker.answering()).await else {
            continue;
        };
        let Some(native) = pairing.filler.talks_in() else {
            continue;
        };
        let connection = connection(&asker.name);
        let key = match super::keys::curator_key(ctx, pairing.filler).await {
            Beneath::Read(key) => key,
            Beneath::Absent => continue,
            Beneath::Escaped => {
                wirings.push(super::keys::refused(connection, pairing.filler));
                continue;
            }
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

/// The curator as this pass tells it: over `indexer.pull` where it speaks the contract,
/// otherwise as the stack's own book curator. Nothing where it speaks the contract and
/// cannot be asked over it.
async fn puller(ctx: &Ctx, fillers: &Fillers, asker: &Filler) -> Option<Puller> {
    match spoken(ctx, asker, pull::CAPABILITY, pull::MAJOR).await {
        Spoken::Over(adapter) => Some(Puller::Over(pull::Adapter(adapter))),
        Spoken::Unanswered => None,
        Spoken::Not => reader(ctx, fillers, asker).map(Puller::Bundled),
    }
}

/// The book curator as a client holding the key lemonfiber minted for it, under the setting
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
///
/// **One that cannot pull from it is left unpaired.** A curator answering `unsupported`
/// reads no aggregator of that native API, which is a pair nothing here connects rather
/// than a fault.
async fn told(
    puller: &Puller,
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
    if let Err(failure) = puller.add(aggregator).await {
        return unreached(asker, aggregator, &failure);
    }
    crate::seed::State::Wired
}

/// A curator that would not take the aggregator, in its own words: unpaired where it
/// cannot pull from one of that native API, failed otherwise.
fn unreached(asker: &str, aggregator: &AggregatorAt, failure: &Failure) -> crate::seed::State {
    match failure {
        Failure::Unsupported { detail, .. } => crate::seed::State::Unmatched {
            reason: format!(
                "{asker} cannot pull its indexers from {}, which it would read in the {} API: \
                 {detail}",
                aggregator.name, aggregator.protocol.0
            ),
        },
        _ => crate::seed::State::Failed {
            detail: failure.to_string(),
        },
    }
}
