//! Holding each curator's quality profiles to the preset the operator chose, through a
//! quality sync that speaks `quality.sync`.
//!
//! The stack's own quality sync is configured by the generated project, which already
//! names every curator and preset, so this asks only a sync speaking the contract: one
//! `apply` per curator it reaches, with the key lemonfiber minted for the curator and the
//! preset chosen for what it files.

use lemonfiber_contract::capabilities::quality::sync;

mod masked;

use super::connecting::{pairings, Cleared, Connection};
use super::restarting::Restarting;
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::model::StackEdit;
use crate::ports::media::Kind;
use crate::ports::service::{Applied, Profiled, Syncing};
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

/// The sync as the core asks it, over `quality.sync`, with the upstream started again
/// where it takes what it applied up only then. Nothing where it cannot be asked over it.
async fn syncer(ctx: &Ctx, fillers: &Fillers, asker: Cleared<'_>) -> Option<Restarting> {
    let answering = asker.answering();
    match spoken(ctx, answering, sync::CAPABILITY, sync::MAJOR).await {
        Spoken::Over(adapter) => Some(Restarting::of(fillers, answering, adapter)),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// What putting every curator's profiles back over a hand edit came to, through each
/// quality sync speaking `quality.sync`: the edit replaced, or on a rehearsal the edit it
/// would replace, masked here whatever the sync masked, or why it was not; nothing where
/// no sync speaks it.
pub(crate) async fn reapplied(
    ctx: &Ctx,
    fillers: &Fillers,
    selection: &Selection,
) -> Option<Result<Option<StackEdit>, String>> {
    let mut asked: Option<Option<StackEdit>> = None;
    for holding in holding(fillers) {
        let Some(syncer) = syncer(ctx, fillers, holding.asker).await else {
            continue;
        };
        match put_back(ctx, &syncer, &holding.curators, selection).await {
            Ok(edit) => asked = Some(asked.flatten().or(edit)),
            Err(why) => return Some(Err(why)),
        }
    }
    asked.map(Ok)
}

/// Put each of `curators` back to the preset `selection` chooses through `syncer`, or on
/// a rehearsal ask what that would replace, its upstream started again once where it
/// takes the profiles up only then.
async fn put_back(
    ctx: &Ctx,
    syncer: &Restarting,
    curators: &[(&Filler, Kind, &Address)],
    selection: &Selection,
) -> Result<Option<StackEdit>, String> {
    let adapter = sync::Adapter(syncer.adapter().clone());
    let mut held = masked::held(ctx)?;
    let (mut replaced, mut restart) = (None, false);
    for (curator, which, at) in curators {
        let api_key = super::keys::minted_key(ctx, curator)
            .ok_or_else(|| format!("lemonfiber holds no key for {}", curator.name))?;
        held.push(api_key.clone());
        let profiled = profiled(*which, at, api_key, selection);
        let edit = if ctx.dry_run {
            adapter.edited(&profiled).await
        } else {
            adapter.reapply(&profiled).await.map(|reapplied| {
                restart |= reapplied.applied == Applied::OnRestart;
                reapplied.replaced
            })
        }
        .map_err(|failure| masked::said(&failure.to_string(), &held))?
        .map(|edit| masked::masked(edit, &held))
        .transpose()?;
        replaced = replaced.or(edit);
    }
    if restart {
        if let State::Failed { detail } = syncer.taken_up(ctx, Ok(Applied::OnRestart)).await {
            return Err(detail);
        }
    }
    Ok(replaced)
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
        let Some(syncer) = syncer(ctx, fillers, holding.asker).await else {
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
            let Some(api_key) = super::keys::minted_key(ctx, curator) else {
                wirings.push(super::skipped(connection, &curator.name));
                continue;
            };
            let profiled = profiled(*which, at, api_key, selection);
            wirings.push(Wiring::settled(
                connection,
                hold(ctx, &syncer, &profiled).await,
            ));
        }
    }
    wirings
}

/// Hand each quality sync holding the curator `curator_id` the key it answers to now,
/// whatever preset it last applied: what replacing that curator's key owes it.
///
/// Answers with each sync's name and how the write came out — none where no sync holds
/// `curator_id`, and none where lemonfiber holds no minted key for it.
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
        let Some(syncer) = syncer(ctx, fillers, holding.asker).await else {
            continue;
        };
        let Some(api_key) = super::keys::minted_key(ctx, curator) else {
            continue;
        };
        let state = match crate::quality::run::load_selection(ctx) {
            Ok(selection) => {
                let applied = sync::Adapter(syncer.adapter().clone())
                    .apply(&profiled(*which, at, api_key, &selection))
                    .await;
                syncer.taken_up(ctx, applied).await
            }
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
async fn hold(ctx: &Ctx, restarting: &Restarting, profiled: &Profiled) -> State {
    let syncer = sync::Adapter(restarting.adapter().clone());
    let applied = match syncer.applied(profiled.which).await {
        Ok(applied) => applied,
        Err(failure) => return crate::seed::unreached(&failure),
    };
    if applied == Some(profiled.preset) {
        return State::AlreadyWired;
    }
    if ctx.dry_run {
        return restarting.would(
            applied.map(|preset| preset.label().to_owned()),
            profiled.preset.label(),
        );
    }
    restarting.taken_up(ctx, syncer.apply(profiled).await).await
}
