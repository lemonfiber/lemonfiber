//! What a pass comes to: each unhealthy item with its reason and strikes, paired by the
//! client's own id, and which of them are due to be acted on.

use std::collections::{BTreeMap, BTreeSet};

use crate::config::Healing;
use crate::model::{Act, Wedged};
use crate::ports::service::{Download, Downloading};
use crate::queue::healing::{classify, in_grace_until, orphaned, struck, Struck};

use super::gathering::Gathered;

/// One unhealthy item, as a pass found it.
pub(super) struct Found {
    /// What its strikes are kept under: the client's id for it in lower case, or its
    /// curator's queue entry where the client names none.
    pub(super) key: String,
    /// The curator it is queued with, by its place among the gathered curators.
    pub(super) curator: Option<usize>,
    /// Its curator's entry, where a curator holds it.
    pub(super) entry: Option<Downloading>,
    /// The client's item, where the client named it.
    pub(super) item: Option<Download>,
    /// The client holding it, where a client named it.
    pub(super) client: Option<String>,
    /// Why it is unhealthy, in words.
    pub(super) why: String,
    /// Its strikes after this pass.
    pub(super) struck: Struck,
}

impl Found {
    /// When it was added: as the client says, or when it was first flagged.
    pub(super) fn added(&self) -> u64 {
        self.item
            .as_ref()
            .and_then(|item| item.added)
            .unwrap_or(self.struck.since)
    }

    /// The release, by its name.
    pub(super) fn release(&self) -> String {
        self.entry
            .as_ref()
            .map(|entry| entry.release.clone())
            .or_else(|| self.item.as_ref().map(|item| item.name.clone()))
            .unwrap_or_default()
    }

    /// What the next run would do to it: drop it, blocklist it and search again where a
    /// curator holds it by a queue entry, and nothing where none does.
    pub(super) fn next(&self) -> Vec<Act> {
        if self.entry.as_ref().and_then(|entry| entry.queued).is_some() {
            vec![Act::Remove, Act::Blocklist, Act::Search]
        } else {
            Vec::new()
        }
    }
}

/// What a pass came to.
pub(super) struct Judged {
    /// Every unhealthy item.
    pub(super) found: Vec<Found>,
    /// Every item's strikes after the pass, by its key.
    pub(super) strikes: BTreeMap<String, Struck>,
}

impl Judged {
    /// The items struck out and past their grace, which a run acts on.
    pub(super) fn due<'j>(&'j self, healing: &Healing, now: u64) -> Vec<&'j Found> {
        self.found
            .iter()
            .filter(|found| found.struck.strikes >= healing.strikes)
            .filter(|found| in_grace_until(found.added(), healing.grace, now).is_none())
            .collect()
    }

    /// Every unhealthy item as the read says it.
    pub(super) fn wedged(&self, gathered: &Gathered, healing: &Healing, now: u64) -> Vec<Wedged> {
        self.found
            .iter()
            .map(|found| {
                let curator = found.curator.and_then(|at| gathered.curators.get(at));
                Wedged {
                    title: found
                        .entry
                        .as_ref()
                        .zip(curator)
                        .and_then(|(entry, curator)| curator.titles.get(&entry.title_id))
                        .cloned(),
                    service: curator
                        .map(|curator| curator.id.clone())
                        .or_else(|| found.client.clone())
                        .unwrap_or_default(),
                    release: found.release(),
                    class: found.struck.wedge,
                    why: found.why.clone(),
                    strikes: found.struck.strikes,
                    added_at: Some(found.added()),
                    in_grace_until: in_grace_until(found.added(), healing.grace, now),
                    next: healing.on.then(|| found.next()),
                }
            })
            .collect()
    }
}

/// Every unhealthy item `gathered` holds, struck against `before`.
///
/// An orphan is judged only where every curator answered, since one that did not may hold
/// it.
pub(super) fn judged(
    gathered: &Gathered,
    before: &BTreeMap<String, Struck>,
    healing: &Healing,
    now: u64,
) -> Judged {
    let mut found = Vec::new();
    let mut paired = BTreeSet::new();
    for (at, curator) in gathered.curators.iter().enumerate() {
        for entry in &curator.entries {
            let id = entry.download_id.as_deref().map(paired_as);
            let item = id.as_ref().and_then(|id| {
                gathered
                    .items
                    .iter()
                    .find(|one| one.download.id.as_deref().map(paired_as).as_ref() == Some(id))
            });
            if let Some(id) = &id {
                paired.insert(id.clone());
            }
            let key = id.unwrap_or_else(|| {
                format!(
                    "{}:{}",
                    curator.id,
                    entry
                        .queued
                        .map_or_else(|| entry.release.clone(), |queued| queued.to_string())
                )
            });
            let wedge = classify(
                Some(entry),
                item.map(|one| &one.download),
                healing.slow,
                now,
            );
            if let Some((wedge, why)) = wedge {
                if let Some(struck) = struck(before.get(&key).copied(), Some(wedge), now) {
                    found.push(Found {
                        key,
                        curator: Some(at),
                        entry: Some(entry.clone()),
                        item: item.map(|one| one.download.clone()),
                        client: item.map(|one| one.client.clone()),
                        why,
                        struck,
                    });
                }
            }
        }
    }
    if !gathered.incomplete {
        for one in &gathered.items {
            let (Some(id), Some(category)) = (&one.download.id, &one.download.category) else {
                continue;
            };
            let id = paired_as(id);
            if paired.contains(&id) || !gathered.categories.contains(category) {
                continue;
            }
            let (wedge, why) = orphaned(category);
            if let Some(struck) = struck(before.get(&id).copied(), Some(wedge), now) {
                found.push(Found {
                    key: id,
                    curator: None,
                    entry: None,
                    item: Some(one.download.clone()),
                    client: Some(one.client.clone()),
                    why,
                    struck,
                });
            }
        }
    }
    let strikes = found
        .iter()
        .map(|found| (found.key.clone(), found.struck))
        .collect();
    Judged { found, strikes }
}

/// A download's id as a client and a curator are paired by: a client names a torrent by
/// its hash in lower case where a curator names it in upper case, so case never tells two
/// apart.
fn paired_as(id: &str) -> String {
    id.to_ascii_lowercase()
}

#[cfg(test)]
mod tests;
