//! A library for each media type the stack files, on every media server.
//!
//! Every service speaking `media.serve` is handed each media type the stack's curators
//! file, with the folder of the data root it is filed in, and adds a library for each
//! whose folder exists and that none reads. It never changes one the operator arranged.
//! A part claimed over `owner.claim` is asked only once lemonfiber holds it.

use std::collections::BTreeSet;

use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_contract::capabilities::owner::claim;

use super::baseline::media_folder;
use super::curating::curators;
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::baseline::Baseline;
use crate::ports::service::{Failure, FiledAt, Furnished, Furnishing};
use crate::seed::{State, Wiring};
use crate::wiring::{Filler, Fillers};

/// What the report calls this connection for the media server `name`.
fn connection(name: &str) -> String {
    format!("{name} given a library for each media type the stack files")
}

/// Each media type the stack's curators file, with the folder it is filed in.
fn filed(fillers: &Fillers) -> Vec<FiledAt> {
    curators(fillers)
        .into_iter()
        .flat_map(|curator| curator.media_types().iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|media_type| FiledAt {
            folder: media_folder(&media_type),
            media_type,
        })
        .collect()
}

/// Give every media server here a library for each media type the stack files.
pub(super) async fn seed_furnishing(
    ctx: &Ctx,
    fillers: &Fillers,
    baseline: &Baseline,
    kept_off: &BTreeSet<String>,
) -> Vec<Wiring> {
    let filed = filed(fillers);
    if filed.is_empty() {
        return Vec::new();
    }
    let mut wirings = Vec::new();
    for speaker in fillers
        .services()
        .filter(|one| one.contracted(serve::CAPABILITY, serve::MAJOR))
    {
        let part = fillers.fronted_by(speaker).unwrap_or(speaker);
        if super::owning::off(fillers, speaker, kept_off) || !held(speaker, part, baseline) {
            continue;
        }
        let state = if ctx.dry_run {
            State::WouldWire {
                yours: None,
                ours: Some(listed(&filed)),
            }
        } else {
            match spoken(ctx, speaker, serve::CAPABILITY, serve::MAJOR).await {
                Spoken::Over(adapter) => {
                    answered(&part.name, serve::Adapter(adapter).furnish(&filed).await)
                }
                Spoken::Unanswered | Spoken::Not => State::Skipped {
                    reason: format!(
                        "{} speaks {} and could not be asked over it",
                        part.name,
                        lemonfiber_contract::spoken(serve::CAPABILITY, serve::MAJOR)
                    ),
                },
            }
        };
        wirings.push(Wiring::settled(connection(&part.name), state));
    }
    wirings
}

/// Whether `part`, which `speaker` speaks for, may be asked: where `speaker` claims it
/// over `owner.claim`, only once lemonfiber holds it.
fn held(speaker: &Filler, part: &Filler, baseline: &Baseline) -> bool {
    !speaker.contracted(claim::CAPABILITY, claim::MAJOR) || super::owning::ours(baseline, &part.id)
}

/// The media types `filed` names, one after another.
fn listed(filed: &[FiledAt]) -> String {
    filed
        .iter()
        .map(|one| one.media_type.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// What the media server `name` answering `answer` comes to.
fn answered(name: &str, answer: Result<Vec<Furnished>, Failure>) -> State {
    match answer {
        Ok(_) => State::Wired,
        Err(Failure::Unsupported { detail, .. }) => State::Unmatched {
            reason: format!("{name} cannot be given a library by lemonfiber: {detail}"),
        },
        Err(failure) => crate::seed::unreached(&failure),
    }
}
