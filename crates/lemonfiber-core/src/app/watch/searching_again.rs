//! Each stuck item acted on once, where the operator has said to: its curator drops the
//! download from its client and blocklists that release, then searches for the title
//! again.
//!
//! Nothing is acted on where the curator's stuck items or its download clients do not
//! answer, and a release acted on once is never acted on again, kept by its fingerprint.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::config::paths::SEARCHED_AGAIN as RECORD;
use crate::journal::{Change, Kind};
use crate::ports::media::Kind as Media;
use crate::ports::service::StuckItem;

use super::super::targets::{curator_targets, fillers_here};
use super::super::Ctx;
use crate::doctor::credentials::Target;

/// The operation each act is journalled under.
pub(crate) const OPERATION: &str = "searching again for a stuck download";

/// The fingerprint of every stuck release acted on.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Searched {
    /// Each release's fingerprint.
    #[serde(default)]
    releases: BTreeSet<String>,
}

/// Whether the operator has said to act on stuck items.
#[must_use]
pub(crate) fn asked_for(ctx: &Ctx) -> bool {
    ctx.settings
        .env_file
        .as_deref()
        .and_then(|path| crate::config::store::read(path).ok())
        .and_then(|file| {
            file.get(crate::config::STUCK_SEARCH_AGAIN_KEY)
                .map(str::to_owned)
        })
        .is_some_and(|value| crate::config::reads_as_on(&value))
}

/// Act once on each stuck item the curators name, where the operator has said to, saying
/// what was acted on, a sentence each.
pub(crate) async fn searched_again(ctx: &Ctx) {
    if ctx.dry_run || !asked_for(ctx) {
        return;
    }
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return;
    };
    let fillers = fillers_here(ctx, &manifest);
    let mut searched: Searched = super::super::record::beside(ctx, RECORD);
    for target in curator_targets(ctx, &fillers).await {
        for said in acted_on(ctx, &target, &mut searched).await {
            ctx.narrator.say(&said).await;
        }
    }
    super::super::record::keep_beside(ctx, RECORD, &searched);
}

/// Each stuck item of `target` not yet acted on, dropped and searched again for.
async fn acted_on(ctx: &Ctx, target: &Target, searched: &mut Searched) -> Vec<String> {
    let (Some(kind), Some(curator)) = (target.kind, target.asked.as_ref()) else {
        return Vec::new();
    };
    let clients_answer = curator
        .test_download_clients()
        .await
        .is_ok_and(|probes| probes.iter().all(|probe| probe.reachable));
    if !clients_answer {
        return Vec::new();
    }
    let Ok(stuck) = curator.stuck_items(kind).await else {
        return Vec::new();
    };
    let mut said = Vec::new();
    for item in stuck {
        let Some((queued, title, release)) = actable(&item, searched) else {
            continue;
        };
        let Ok(dropped) = curator.unqueue(kind, queued, true).await else {
            continue;
        };
        journalled(ctx, &target.id, kind, title, &dropped);
        searched.releases.insert(release);
        searched.releases.insert(dropped);
        let searching = curator.search(kind, title).await.is_ok();
        said.push(if searching {
            format!(
                "{} dropped {} and searches for it again",
                target.name, item.title
            )
        } else {
            format!(
                "{} dropped {} and would not search for it again",
                target.name, item.title
            )
        });
    }
    said
}

/// The queue entry, title and release of `item`, where it names all three and its release
/// has not been acted on.
fn actable(item: &StuckItem, searched: &Searched) -> Option<(i64, i64, String)> {
    let release = item.release.clone()?;
    (!searched.releases.contains(&release)).then_some(())?;
    Some((item.queued?, item.title_id?, release))
}

/// Journal one act against the curator `service`, best effort as every act on the watch's
/// pace is.
fn journalled(ctx: &Ctx, service: &str, kind: Media, title: i64, release: &str) {
    let change = Change {
        at: ctx.stamp(),
        operation: OPERATION.to_owned(),
        target: service.to_owned(),
        kind: Kind::Unqueued {
            kind,
            title,
            release: release.to_owned(),
        },
    };
    if let Some(paths) = super::super::targets::layout(ctx) {
        let _ = super::super::recover::journalled(&paths.journal(), std::slice::from_ref(&change));
    }
}

#[cfg(test)]
mod tests;
