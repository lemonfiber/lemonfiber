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

use lemonfiber_manifest::Service;

use super::Ctx;
use crate::ports::service::{Aggregator, Aggregators as _};

/// What this connection is called where it is reported.
const CONNECTION: &str = "Indexers into Bindery";

/// What this connection asks the stack for: a service that runs a search across the
/// indexers it holds and answers with what they returned.
const SEARCHES: &str = "indexer.search";

/// Tell the book \*arr about the aggregator, where the stack has both.
///
/// Nothing where either is absent, or where the book \*arr has no key yet — the key is
/// minted on the run that first reaches it, and a service started before that is
/// completed by a later run rather than failed.
pub(super) async fn seed_aggregators(
    ctx: &Ctx,
    services: &[Service],
    project: Option<&std::path::Path>,
    filled: &std::collections::BTreeMap<String, Vec<String>>,
) -> Vec<crate::seed::Wiring> {
    let Some(client) = super::super::targets::bindery_reader(ctx, services) else {
        return Vec::new();
    };
    let Some(aggregator) = aggregator_to_pull_from(ctx, services, project, filled).await else {
        return Vec::new();
    };

    vec![crate::seed::Wiring::settled(
        CONNECTION.to_owned(),
        told(&client, &aggregator, ctx.dry_run).await,
    )]
}

/// The aggregator as the book \*arr needs to be told about it: where it is on the
/// stack's own network, and its key.
///
/// A container name rather than a loopback address, because the service reading it is
/// a container beside it.
///
/// Which service that is comes from what the stack says this link asks for, not from a
/// name written here. Two of the stack's own services answer as an indexer, and which
/// of them fills the ask is the manifest's default until the operator substitutes —
/// at which point this reaches the other one with nothing here changed, which is the
/// whole of what asking rather than naming buys.
async fn aggregator_to_pull_from(
    ctx: &Ctx,
    services: &[Service],
    project: Option<&std::path::Path>,
    filled: &std::collections::BTreeMap<String, Vec<String>>,
) -> Option<Aggregator> {
    let [chosen] = filled.get(SEARCHES)?.as_slice() else {
        return None;
    };
    let aggregator = services.iter().find(|service| &service.id == chosen)?;
    let port = aggregator.port?;
    let target = super::target_for(aggregator, project?)?;
    let key = super::arrs::read_servarr_key(ctx, &target.config).await?;
    Some(Aggregator {
        name: aggregator.name.clone(),
        url: format!("http://{}:{port}", aggregator.id),
        key,
    })
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
