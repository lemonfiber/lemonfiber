//! Where each download client files each category's completed downloads.
//!
//! Every category a curator hands a client is filed under the one completed folder both
//! clients share, `/data/downloads/complete/<category>`, so a curator's downloads land in
//! one folder whichever client fetched them. Each client is told over `download.usenet`
//! or `download.torrent`, through this build's adapter for an assembled client and over
//! its contract for a plugin's, and read back.
//!
//! **A folder the operator chose is theirs.** One a client holds for a category, other
//! than the one lemonfiber last filed there, is kept and said as drift. Where lemonfiber
//! has filed nothing there yet, what the client holds is its own default rather than a
//! choice anybody made, so the category is filed.

use std::collections::{BTreeMap, BTreeSet};

use super::baseline::DATA_ROOT;
use super::clients::category_for;
use super::connecting::{pairings, Connection};
use super::Ctx;
use crate::baseline::Baseline;
use crate::ports::service::Filed;
use crate::seed::{State, Wiring};
use crate::wiring::Fillers;

/// The folder a category's completed downloads land in, whichever client fetched them.
#[must_use]
pub(crate) fn completed(category: &str) -> String {
    format!("{DATA_ROOT}/downloads/complete/{category}")
}

/// The field of the baseline a category's completed folder is recorded under.
fn field(category: &str) -> String {
    format!("completed:{category}")
}

/// Every download client a curator hands downloads to, by id, with the categories it is
/// handed them under.
fn categories(fillers: &Fillers) -> BTreeMap<String, BTreeSet<String>> {
    let mut wanted: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for pairing in pairings(fillers) {
        let Ok((Connection::DownloadClient(_), _, _)) = &pairing.made else {
            continue;
        };
        let Some(category) = pairing
            .asker
            .media_types
            .first()
            .and_then(|media| category_for(media))
        else {
            continue;
        };
        wanted
            .entry(pairing.filler.id.clone())
            .or_default()
            .insert(category.value);
    }
    wanted
}

/// Tell every download client a curator hands downloads to where each of its categories
/// completes, and say how each stands.
pub(super) async fn seed_completed_folders(
    ctx: &Ctx,
    fillers: &Fillers,
    baseline: &mut Baseline,
) -> Vec<Wiring> {
    let wanted = categories(fillers);
    let mut wirings = Vec::new();
    for target in crate::app::targets::download_targets(ctx, fillers).await {
        let Some(categories) = wanted.get(&target.service) else {
            continue;
        };
        let name = fillers
            .service(&target.service)
            .map_or(target.service.as_str(), |filler| filler.name.as_str());
        let client = target.client(ctx);
        let state = filed(ctx, client.filing(), &target.service, categories, baseline).await;
        wirings.push(Wiring::settled(format!("{name} completed folders"), state));
    }
    wirings
}

/// What became of filing `categories` on one client recorded under `id`.
async fn filed(
    ctx: &Ctx,
    client: &dyn crate::ports::service::Filing,
    id: &str,
    categories: &BTreeSet<String>,
    baseline: &mut Baseline,
) -> State {
    let held = match client.filed().await {
        Ok(held) => by_category(held),
        Err(failure) => return crate::seed::unreached(&failure),
    };
    let mut owed = Vec::new();
    let mut drifted = false;
    for category in categories {
        let wanted = completed(category);
        let holds = held.get(category).map(String::as_str);
        if holds == Some(wanted.as_str()) {
            continue;
        }
        match baseline.expected(id, &field(category)) {
            Some(last) if holds.is_some() && holds != Some(last) => drifted = true,
            _ => owed.push(Filed {
                category: category.clone(),
                folder: wanted,
            }),
        }
    }
    if owed.is_empty() {
        record(ctx, baseline, id, categories, &held);
        return if drifted {
            State::Drifted
        } else {
            State::AlreadyWired
        };
    }
    if ctx.dry_run {
        return State::WouldWire {
            yours: Some(described(owed.iter().map(|one| {
                (
                    one.category.as_str(),
                    held.get(&one.category).map_or("nothing", String::as_str),
                )
            }))),
            ours: Some(described(
                owed.iter()
                    .map(|one| (one.category.as_str(), one.folder.as_str())),
            )),
        };
    }
    if let Err(failure) = client.file_under(&owed).await {
        return State::Failed {
            detail: format!("the completed folders could not be set: {failure}"),
        };
    }
    let now = match client.filed().await {
        Ok(now) => by_category(now),
        Err(failure) => {
            return State::Failed {
                detail: format!("the completed folders could not be read back: {failure}"),
            }
        }
    };
    record(ctx, baseline, id, categories, &now);
    if let Some(missed) = owed
        .iter()
        .find(|one| now.get(&one.category) != Some(&one.folder))
    {
        return State::Failed {
            detail: format!(
                "{} was set to complete into {} and the client files it under {}",
                missed.category,
                missed.folder,
                now.get(&missed.category).map_or("nothing", String::as_str)
            ),
        };
    }
    if drifted {
        State::Drifted
    } else {
        State::Wired
    }
}

/// Record every category the client now files where lemonfiber files it.
fn record(
    ctx: &Ctx,
    baseline: &mut Baseline,
    id: &str,
    categories: &BTreeSet<String>,
    held: &BTreeMap<String, String>,
) {
    let at = ctx.stamp();
    for category in categories {
        let wanted = completed(category);
        if held.get(category) == Some(&wanted) {
            baseline.record(id, &field(category), &wanted, &at);
        }
    }
}

/// What a client files, by category.
fn by_category(filed: Vec<Filed>) -> BTreeMap<String, String> {
    filed
        .into_iter()
        .map(|one| (one.category, one.folder))
        .collect()
}

/// Each category and its folder, as one line of a report.
fn described<'a>(folders: impl Iterator<Item = (&'a str, &'a str)>) -> String {
    folders
        .map(|(category, folder)| format!("{category} under {folder}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests;
