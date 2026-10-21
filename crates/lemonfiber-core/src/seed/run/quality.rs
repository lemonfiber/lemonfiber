//! Holding each curator's quality profiles to the preset the operator chose, through a
//! quality sync that speaks `quality.sync`.
//!
//! The stack's own quality sync is configured by the generated project, which already
//! names every curator and preset, so this asks only a sync speaking the contract: one
//! `apply` per curator it reaches, with the curator's own key and the preset chosen for
//! what it files.

use lemonfiber_contract::capabilities::quality::sync;

use super::connecting::{pairings, Cleared, Connection};
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::ports::filesystem::Beneath;
use crate::ports::media::Kind;
use crate::ports::service::{Profiled, Syncing};
use crate::quality::Selection;
use crate::seed::{State, Wiring};
use crate::wiring::{Address, Filler, Fillers};

/// One quality sync, and every curator it reaches with where it reaches each.
struct Holding<'a> {
    /// The sync that asks.
    asker: Cleared<'a>,
    /// Each curator, as the sync files it, and where it reaches it.
    curators: Vec<(&'a Filler, Kind, &'a Address)>,
}

/// Every quality sync and the curators lemonfiber hands it.
fn holding(fillers: &Fillers) -> Vec<Holding<'_>> {
    let mut found: Vec<Holding<'_>> = Vec::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::Quality(which), at, asker)) = pairing.made else {
            continue;
        };
        match found.iter_mut().find(|one| one.asker.id == asker.id) {
            Some(one) => one.curators.push((pairing.filler, which, at)),
            None => found.push(Holding {
                asker,
                curators: vec![(pairing.filler, which, at)],
            }),
        }
    }
    found
}

/// The sync as the core asks it, over `quality.sync`. Nothing where it cannot be asked
/// over it.
async fn syncer(ctx: &Ctx, asker: Cleared<'_>) -> Option<Box<dyn Syncing>> {
    match spoken(ctx, &asker, sync::CAPABILITY, sync::MAJOR).await {
        Spoken::Over(adapter) => Some(Box::new(sync::Adapter(adapter))),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// What a curator's profiles held by a quality sync are called where they are reported.
fn for_quality(curator: &str) -> String {
    format!("{curator}'s quality profiles")
}

/// Hold every curator a contracted quality sync reaches to the preset chosen for it.
///
/// A choice on record that cannot be read is reported on every curator rather than
/// guessed at, the stance every other reader of it takes.
pub(super) async fn seed_quality(ctx: &Ctx, fillers: &Fillers) -> Vec<Wiring> {
    let mut wirings = Vec::new();
    for holding in holding(fillers) {
        let Some(syncer) = syncer(ctx, holding.asker).await else {
            continue;
        };
        let selection = crate::quality::run::load_selection(ctx);
        for (curator, which, at) in &holding.curators {
            let connection = for_quality(&curator.name);
            let selection = match &selection {
                Ok(selection) => selection,
                Err(problem) => {
                    wirings.push(Wiring::settled(
                        connection,
                        State::Failed {
                            detail: problem.summary.clone(),
                        },
                    ));
                    continue;
                }
            };
            let api_key = match super::keys::curator_key(ctx, curator).await {
                Beneath::Read(key) => key,
                Beneath::Absent => {
                    wirings.push(super::skipped(connection, &curator.name));
                    continue;
                }
                Beneath::Escaped => {
                    wirings.push(super::keys::refused(connection, curator));
                    continue;
                }
            };
            let profiled = profiled(*which, at, api_key, selection);
            wirings.push(Wiring::settled(
                connection,
                hold(syncer.as_ref(), &profiled, ctx.dry_run).await,
            ));
        }
    }
    wirings
}

/// Hand each quality sync holding the curator `curator_id` the key it answers to now,
/// whatever preset it last applied: what replacing that curator's key owes it.
///
/// Answers with each sync's name and how the write came out — none where no sync holds
/// `curator_id`, and none from one where a key is not written yet.
pub(crate) async fn reprofile(
    ctx: &Ctx,
    fillers: &Fillers,
    curator_id: &str,
) -> Vec<(String, State)> {
    let mut found = Vec::new();
    for holding in holding(fillers) {
        let Some((curator, which, at)) = holding
            .curators
            .iter()
            .find(|(curator, _, _)| curator.id == curator_id)
        else {
            continue;
        };
        let Some(syncer) = syncer(ctx, holding.asker).await else {
            continue;
        };
        let api_key = match super::keys::curator_key(ctx, curator).await {
            Beneath::Read(key) => key,
            Beneath::Absent => continue,
            Beneath::Escaped => {
                found.push((holding.asker.name.clone(), super::keys::refusal(curator)));
                continue;
            }
        };
        let state = match crate::quality::run::load_selection(ctx) {
            Ok(selection) => match syncer
                .apply(&profiled(*which, at, api_key, &selection))
                .await
            {
                Ok(()) => State::Wired,
                Err(failure) => crate::seed::unreached(&failure),
            },
            Err(problem) => State::Failed {
                detail: problem.summary,
            },
        };
        found.push((holding.asker.name.clone(), state));
    }
    found
}

/// The curator of `which` at `at`, answering to `api_key`, held to the preset `selection`
/// chooses for what it files.
fn profiled(which: Kind, at: &Address, api_key: String, selection: &Selection) -> Profiled {
    Profiled {
        which,
        host: at.host.clone(),
        port: at.port,
        api_key,
        preset: selection.for_type(which.media_type()),
    }
}

/// Hold one curator to its preset, leaving it alone where the sync already does.
///
/// Read first, so a sync already holding the preset is not written again to report a
/// change to nothing. A replaced key reaches it through [`reprofile`], which the key's
/// replacement runs.
async fn hold(syncer: &dyn Syncing, profiled: &Profiled, rehearsing: bool) -> State {
    let applied = match syncer.applied(profiled.which).await {
        Ok(applied) => applied,
        Err(failure) => return crate::seed::unreached(&failure),
    };
    if applied == Some(profiled.preset) {
        return State::AlreadyWired;
    }
    if rehearsing {
        return State::WouldWire {
            yours: applied.map(|preset| preset.label().to_owned()),
            ours: Some(profiled.preset.label().to_owned()),
        };
    }
    if let Err(failure) = syncer.apply(profiled).await {
        return crate::seed::unreached(&failure);
    }
    State::Wired
}
