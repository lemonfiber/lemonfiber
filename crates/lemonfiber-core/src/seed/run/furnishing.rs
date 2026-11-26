//! A library for each media type the stack files or its media server serves, on every
//! media server.
//!
//! Every service speaking `media.serve` is handed each media type the stack's curators
//! file together with each it declares in its own `media_types`, once each, with the
//! folder of the data root it is filed in, and adds a library for each whose folder
//! exists and that none reads. It never changes one the operator arranged. Each of those
//! folders is made beneath the data root first, so a media server whose media no curator
//! files is still given its libraries. A part claimed over `owner.claim` is asked only
//! once lemonfiber holds it. A media type outside the stack manifest's vocabulary forms
//! no folder and is reported, never handed over.

use std::collections::BTreeSet;
use std::path::Path;

use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_contract::capabilities::owner::claim;

use super::baseline::{media_folder, DATA_ROOT};
use super::curating::curators;
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::baseline::Baseline;
use crate::ports::media::MEDIA;
use crate::ports::service::{Failure, FiledAt, Furnished, Furnishing, RootFolder};
use crate::seed::{State, Wiring};
use crate::wiring::{Filler, Fillers};

/// Each media type with the folder of the data root it is filed in.
type Folders = BTreeSet<(String, String)>;

/// A media server to ask: the service speaking `media.serve`, the part it speaks for,
/// and the media types it is handed.
type Asked<'a> = (&'a Filler, &'a Filler, Folders);

/// What the report calls this connection for the media server `name`.
fn connection(name: &str) -> String {
    format!("{name} given a library for each media type the stack files or it serves")
}

/// Each of `media_types`, which `name` is `doing`, put in `folders` with the folder it
/// is filed in; each outside the stack manifest's vocabulary forms no folder and is
/// reported.
fn foldered(name: &str, doing: &str, media_types: &[String], folders: &mut Folders) -> Vec<Wiring> {
    let mut refused = Vec::new();
    for media_type in media_types {
        match media_folder(media_type) {
            Some(folder) => {
                folders.insert((media_type.clone(), folder));
            }
            None => refused.push(Wiring::settled(
                format!("{name} {doing} {media_type:?}"),
                State::Unmatched {
                    reason: format!(
                        "{media_type:?} is not one of {}, so no media server is handed a \
                         folder for it",
                        MEDIA.join(", ")
                    ),
                },
            )),
        }
    }
    refused
}

/// Each media type the stack's curators file, with the folder it is filed in, and each
/// curator's media type outside the stack manifest's vocabulary, which no folder is built
/// for.
fn filed(fillers: &Fillers) -> (Folders, Vec<Wiring>) {
    let mut folders = BTreeSet::new();
    let mut refused = Vec::new();
    for curator in curators(fillers) {
        refused.extend(foldered(
            curator.name(),
            "filing",
            curator.media_types(),
            &mut folders,
        ));
    }
    (folders, refused)
}

/// Give every media server here a library for each media type the stack files or it
/// serves, making each one's folder beneath `data_root` first.
pub(super) async fn seed_furnishing(
    ctx: &Ctx,
    fillers: &Fillers,
    baseline: &Baseline,
    kept_off: &BTreeSet<String>,
    data_root: Option<&Path>,
) -> Vec<Wiring> {
    let (filed, mut wirings) = filed(fillers);
    let mut asking: Vec<Asked<'_>> = Vec::new();
    for speaker in fillers
        .services()
        .filter(|one| one.contracted(serve::CAPABILITY, serve::MAJOR))
    {
        let part = fillers.fronted_by(speaker).unwrap_or(speaker);
        if super::owning::off(fillers, speaker, kept_off) || !held(speaker, part, baseline) {
            continue;
        }
        let mut serves = filed.clone();
        wirings.extend(foldered(
            &part.name,
            "serving",
            &speaker.media_types,
            &mut serves,
        ));
        if !serves.is_empty() {
            asking.push((speaker, part, serves));
        }
    }
    if !ctx.dry_run {
        wirings.extend(made(ctx, data_root, &asking).await);
    }
    for (speaker, part, serves) in asking {
        let handed: Vec<FiledAt> = serves
            .into_iter()
            .map(|(media_type, folder)| FiledAt { media_type, folder })
            .collect();
        let state = if ctx.dry_run {
            State::WouldWire {
                yours: None,
                ours: Some(listed(&handed)),
            }
        } else {
            match spoken(ctx, speaker, serve::CAPABILITY, serve::MAJOR).await {
                Spoken::Over(adapter) => {
                    answered(&part.name, serve::Adapter(adapter).furnish(&handed).await)
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

/// The folder beneath `data_root` for each media type a media server in `asking` is
/// handed, made once each; one that cannot be made is reported with the platform's
/// reason. Nothing is made where no data root is known.
async fn made(ctx: &Ctx, data_root: Option<&Path>, asking: &[Asked<'_>]) -> Vec<Wiring> {
    let Some(data_root) = data_root else {
        return Vec::new();
    };
    let folders: BTreeSet<&(String, String)> =
        asking.iter().flat_map(|(_, _, serves)| serves).collect();
    let hosts = folders.into_iter().filter_map(|(media_type, folder)| {
        let at = RootFolder {
            path: folder.clone(),
            media_type: media_type.clone(),
        };
        crate::seed::on_host(&at, DATA_ROOT, data_root).map(|host| (media_type, host))
    });
    let mut unmade = Vec::new();
    for (media_type, host) in hosts {
        if let Err(fault) = ctx.seams.filesystem.make_beneath(&host, data_root).await {
            unmade.push(Wiring::settled(
                format!("the folder for {media_type} beneath the data root"),
                State::Failed {
                    detail: format!(
                        "the directory {} could not be made: {}",
                        host.display(),
                        fault.message
                    ),
                },
            ));
        }
    }
    unmade
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
