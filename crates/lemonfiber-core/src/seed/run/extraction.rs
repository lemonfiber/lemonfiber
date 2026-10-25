//! Telling an extractor that speaks `archive.extract` whose completed downloads it
//! unpacks.
//!
//! Every curator it asks for is handed at once, in place of those it watches, with the
//! curator's own key and the folder its downloads complete into: the one the download
//! clients read back for the curator's category. The extractor hands each finished
//! extraction back to that curator itself.
//!
//! **One folder per curator.** Both clients file a category under one shared folder, so
//! the curator's is that folder. Where the operator moved one client's folder, the
//! shared folder is still the one handed while any client files the category there,
//! since that is where lemonfiber files it; where none does, the operator's folder is.
//!
//! The stack's own extractor is configured by the generated project and speaks no
//! contract, so only one that speaks it is told anything here.

use lemonfiber_contract::capabilities::archive::extract;

use super::connecting::{pairings, Cleared, Connection};
use super::filing::{completed, Completed};
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::ports::filesystem::Beneath;
use crate::ports::service::{Curated, Extracting};
use crate::seed::{State, Wiring};
use crate::wiring::{Address, Filler, Fillers};

/// One extractor, and every curator it asks for with where it reaches each.
struct Holding<'a> {
    /// The extractor.
    asker: Cleared<'a>,
    /// Each curator, where the extractor reaches it, the media it files and the category
    /// its downloads are filed under.
    curators: Vec<(&'a Filler, &'a Address, String, String)>,
}

/// Every extractor and the curators lemonfiber hands it.
fn holding(fillers: &Fillers) -> Vec<Holding<'_>> {
    let mut found: Vec<Holding<'_>> = Vec::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::Extraction { media, category }, at, asker)) = pairing.made else {
            continue;
        };
        let curator = (pairing.filler, at, media, category);
        match found.iter_mut().find(|one| one.asker.id == asker.id) {
            Some(one) => one.curators.push(curator),
            None => found.push(Holding {
                asker,
                curators: vec![curator],
            }),
        }
    }
    found
}

/// The folder a curator's downloads filed under `category` complete into, as the
/// clients read it back in `held`.
fn folder(category: &str, held: &Completed) -> String {
    let shared = completed(category);
    match held.get(category) {
        Some(folders) if !folders.contains(&shared) => folders.first().cloned().unwrap_or(shared),
        _ => shared,
    }
}

/// Tell every extractor speaking `archive.extract` each curator it asks for, and say how
/// each stands.
pub(super) async fn seed_extraction(ctx: &Ctx, fillers: &Fillers, held: &Completed) -> Vec<Wiring> {
    let mut wirings = Vec::new();
    for holding in holding(fillers) {
        let connection = format!("{} watching every curator", holding.asker.name);
        let extractor = match spoken(
            ctx,
            holding.asker.answering(),
            extract::CAPABILITY,
            extract::MAJOR,
        )
        .await
        {
            Spoken::Over(adapter) => extract::Adapter(adapter),
            Spoken::Unanswered | Spoken::Not => continue,
        };
        let mut curated = Vec::new();
        let mut withheld = None;
        for (curator, at, media, category) in &holding.curators {
            match super::keys::curator_key(ctx, curator).await {
                Beneath::Read(key) => curated.push(Curated {
                    media_type: media.clone(),
                    address: at.url(),
                    key,
                    folder: folder(category, held),
                }),
                Beneath::Absent => {
                    withheld = Some(super::skipped(connection.clone(), &curator.name));
                    break;
                }
                Beneath::Escaped => {
                    withheld = Some(super::keys::refused(connection.clone(), curator));
                    break;
                }
            }
        }
        if let Some(wiring) = withheld {
            wirings.push(wiring);
            continue;
        }
        wirings.push(Wiring::settled(
            connection,
            watched(&extractor, &curated, ctx.dry_run).await,
        ));
    }
    wirings
}

/// Hand the extractor every curator in `curated` at once.
///
/// Every curator together or none, because what it is handed replaces what it watches: a
/// curator left out of one call is a curator it stops watching.
async fn watched(extractor: &dyn Extracting, curated: &[Curated], rehearsing: bool) -> State {
    if rehearsing {
        return State::WouldWire {
            yours: None,
            ours: Some(
                curated
                    .iter()
                    .map(|one| format!("{} in {}", one.media_type, one.folder))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        };
    }
    match extractor.watch(curated).await {
        Ok(()) => State::Wired,
        Err(failure) => crate::seed::unreached(&failure),
    }
}
