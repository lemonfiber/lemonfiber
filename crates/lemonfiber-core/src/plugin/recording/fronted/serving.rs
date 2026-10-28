//! What a recording serves: the upstream's container and the adapter's beside it, or the
//! one container of a service that is its own upstream, and every party its cases point
//! it at.

use std::path::Path;

use lemonfiber_contract::conformance::filed;

use super::super::served::{settled, settled_as};
use super::super::Recorder;
use super::preparing::Prepared;
use super::speaking::contracts;
use super::{enveloping, Faulted, Fronting};
use crate::plugin::installed::Placed;
use crate::plugin::program::Operator;
use crate::ports::serving::{Bound, Serve};

/// The upstream's container reaching `reaching`, and the adapter's beside it where it is
/// apart from its upstream: as lemonfiber runs it, run as whoever owns its copy, from the
/// image a check names in place of its pin, its upstream's copy mounted where it shares
/// its upstream's configuration. An adapter that is its own upstream is the
/// one container, run as the adapter.
pub(super) fn serves(
    recorder: Recorder<'_>,
    upstream: &Placed,
    adapter: Fronting<'_>,
    prepared: &Prepared,
    reaching: Vec<String>,
) -> (Serve, Option<Serve>) {
    let operator = prepared
        .held()
        .owner()
        .map_or_else(Operator::recording, |(uid, gid)| Operator {
            ids: (uid.to_string(), gid.to_string()),
            ..Operator::recording()
        });
    let alone = adapter.placed.service == upstream.service;
    let shared = adapter
        .placed
        .fields
        .shares_config
        .as_ref()
        .filter(|_| !alone && adapter.placed.fronts.as_deref() == Some(upstream.service.as_str()))
        .map(|at| Bound {
            source: prepared.copy.path().to_path_buf(),
            target: at.clone(),
        });
    let adapting = |data: Option<&Path>| {
        let started = settled_as(adapter.placed, prepared.held(), data, &operator);
        let mut binds = started.binds.clone();
        binds.extend(shared.clone());
        Serve {
            image: recorder
                .taking
                .adapter()
                .map_or_else(|| started.image.clone(), str::to_owned),
            binds,
            ..started
        }
    };
    let settled = if alone {
        adapting(prepared.data.as_deref())
    } else {
        settled(upstream, &prepared.copy, prepared.data.as_deref())
    };
    let serve = Serve {
        alias: Some(upstream.service.clone()),
        reaching,
        env: settled.env.iter().chain(&prepared.env).cloned().collect(),
        ..settled
    };
    (serve, (!alone).then(|| adapting(None)))
}

/// Every party the cases of each contract `adapter` speaks point the upstream at, by the
/// host they name it on, each answering until it is dropped.
pub(super) async fn parties(
    recorder: Recorder<'_>,
    adapter: Fronting<'_>,
    upstream: &Placed,
    enveloped: bool,
) -> Result<(Vec<String>, Vec<Box<dyn crate::ports::serving::Answering>>), Faulted> {
    let asked: Vec<String> = contracts(adapter.service)
        .iter()
        .flat_map(|(_, capability)| filed(capability, &upstream.media_types))
        .map(|case| case.asks.to_string())
        .collect();
    let mut reaching = Vec::new();
    let mut answering = Vec::new();
    for party in lemonfiber_contract::capabilities::parties(&upstream.media_types) {
        if !asked.iter().any(|asks| asks.contains(party.host)) {
            continue;
        }
        let answered = recorder
            .serving
            .stand_in(&party.answering)
            .await
            .map_err(|unran| Faulted::new(party.named, unran.to_string(), Vec::new()))?;
        reaching.push(party.host.to_owned());
        answering.push(answered);
    }
    for (host, stand_in) in enveloped.then(enveloping::parties).into_iter().flatten() {
        let answered = recorder
            .serving
            .stand_in(&stand_in)
            .await
            .map_err(|unran| Faulted::new(host, unran.to_string(), Vec::new()))?;
        reaching.push(host.to_owned());
        answering.push(answered);
    }
    Ok((reaching, answering))
}
