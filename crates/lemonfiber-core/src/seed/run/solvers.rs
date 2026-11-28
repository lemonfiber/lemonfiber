//! Telling the indexer the challenge solver it fetches an indexer site behind bot
//! protection through.
//!
//! The solver is told to the indexer in the API it answers in, under a tag: the indexer
//! sends an indexer site through the solver only where the site carries that tag, and
//! which sites stand behind bot protection is the operator's to say, so no site is given
//! it here. The indexer is asked over `indexer.search`, and one that answers it cannot
//! fetch through a solver of that API is left unpaired. The indexer tries a solver it is
//! told about at once, so a solver asked over `indexer.proxy` is given until it answers,
//! as a service coming up from its first start is, before the indexer is told about it.
//! An indexer told about a solver holds it only once its own try of the solver is done,
//! and a try that outlasts the call leaves the call unanswered, so an indexer that did not
//! answer being told is read until it holds the solver rather than told again.

use std::time::Duration;

use lemonfiber_contract::capabilities::indexer::{proxy, search, SOLVER_TAG};

use super::connecting::{pairings, Connection};
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::patience::Patience;
use crate::ports::service::{Failure, KnownSolver, Protocol, SolverAt, Solvers as _, Solving as _};
use crate::wiring::{Filler, Fillers};

/// How long an indexer that did not answer being told about a solver is read until it
/// holds it: past its own try of the solver, whose first answer waits on the solver's
/// browser starting.
const HOLDING: Patience = Patience {
    between: Duration::from_secs(5),
    asks: 24,
};

/// What this connection is called where it is reported.
fn connection(solver: &str, indexer: &str) -> String {
    format!("{solver} into {indexer}")
}

/// Tell each indexer about the challenge solver its ask for one settles on, the stack's or
/// a plugin's, reached where the solver says it listens on the stack's network.
///
/// A pair nothing here connects is reported by the table, never dropped.
pub(super) async fn seed_solvers(ctx: &Ctx, fillers: &Fillers) -> Vec<crate::seed::Wiring> {
    let mut wirings = Vec::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::Solver, at, asker)) = pairing.made else {
            continue;
        };
        let (Some(indexer), Some(native)) = (
            indexer(ctx, asker.answering()).await,
            pairing.filler.talks_in(),
        ) else {
            continue;
        };
        let solver = SolverAt {
            name: pairing.filler.name.clone(),
            url: at.url(),
            protocol: Protocol(native.name().to_owned()),
            tag: SOLVER_TAG.to_owned(),
        };
        let state = if ctx.dry_run || answering(ctx, fillers, pairing.filler).await {
            told(&indexer, &asker.name, &solver, ctx.dry_run).await
        } else {
            crate::seed::State::Failed {
                detail: format!(
                    "{} did not answer within {} seconds of being asked, so {} was not told \
                     about it",
                    solver.name,
                    crate::patience::STARTING.longest().as_secs(),
                    asker.name
                ),
            }
        };
        wirings.push(crate::seed::Wiring::settled(
            connection(&solver.name, &asker.name),
            state,
        ));
    }
    wirings
}

/// Whether `solver` answers, asked through the adapter fronting it as a service coming up
/// from its first start is; one asked over no contract is not asked.
async fn answering(ctx: &Ctx, fillers: &Fillers, solver: &Filler) -> bool {
    let speaker = fillers.fronting(solver).unwrap_or(solver);
    let Spoken::Over(adapter) = spoken(ctx, speaker, proxy::CAPABILITY, proxy::MAJOR).await else {
        return true;
    };
    let adapter = proxy::Adapter(adapter);
    let answers = |answered: &Result<bool, Failure>| matches!(answered, Ok(true));
    answers(&adapter.answers().await)
        || answers(
            &crate::patience::STARTING
                .until(|| adapter.answers(), answers)
                .await,
        )
}

/// The indexer as this pass tells it, over `indexer.search`: nothing where it does not
/// speak the contract or cannot be asked over it.
async fn indexer(ctx: &Ctx, asker: &Filler) -> Option<search::Adapter> {
    match spoken(ctx, asker, search::CAPABILITY, search::MAJOR).await {
        Spoken::Over(adapter) => Some(search::Adapter(adapter)),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// Point the indexer at the solver under the tag, leaving it alone where it already
/// fetches through a solver at that address under that tag.
async fn told(
    indexer: &search::Adapter,
    asker: &str,
    solver: &SolverAt,
    rehearsing: bool,
) -> crate::seed::State {
    let held = match indexer.proxies().await {
        Ok(held) => held,
        Err(failure) => return unreached(asker, solver, &failure),
    };
    if holds(&held, solver) {
        return crate::seed::State::AlreadyWired;
    }
    if rehearsing {
        return crate::seed::State::WouldWire {
            yours: None,
            ours: Some(solver.url.clone()),
        };
    }
    match indexer.add_proxy(solver).await {
        Ok(()) => crate::seed::State::Wired,
        Err(failure @ Failure::Unavailable { .. }) => {
            held_later(indexer, asker, solver, &failure).await
        }
        Err(failure) => unreached(asker, solver, &failure),
    }
}

/// Whether `held` lists a solver at the address of `solver` under its tag.
fn holds(held: &[KnownSolver], solver: &SolverAt) -> bool {
    held.iter()
        .any(|known| known.url == solver.url && known.tags.contains(&solver.tag))
}

/// What an indexer that did not answer being told about `solver` comes to: wired once it
/// holds the solver, read as [`HOLDING`] allows, and `failure` where it never does.
async fn held_later(
    indexer: &search::Adapter,
    asker: &str,
    solver: &SolverAt,
    failure: &Failure,
) -> crate::seed::State {
    let listing = |read: &Result<Vec<KnownSolver>, Failure>| {
        read.as_ref().is_ok_and(|held| holds(held, solver))
    };
    if listing(&HOLDING.until(|| indexer.proxies(), listing).await) {
        crate::seed::State::Wired
    } else {
        unreached(asker, solver, failure)
    }
}

/// An indexer that would not take the solver, in its own words: unpaired where it cannot
/// fetch through a solver of that API, and otherwise as any other failure.
fn unreached(asker: &str, solver: &SolverAt, failure: &Failure) -> crate::seed::State {
    match failure {
        Failure::Unsupported { detail, .. } => crate::seed::State::Unmatched {
            reason: format!(
                "{asker} cannot fetch through {}, which it would ask in the {} API: {detail}",
                solver.name, solver.protocol.0
            ),
        },
        _ => crate::seed::unreached(failure),
    }
}
