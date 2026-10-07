//! The stream's source in a run: the gather the dashboard already runs on.
//!
//! Not a gather written for the web. The terminal screen and the browser read
//! the same figures from the same call, so the two cannot grade the same stack
//! differently — which is the whole reason this is a source rather than a second
//! assembly of the same panels.
//!
//! What the last gather said is kept, because that is what the gather wants: a
//! source that answered a moment ago and did not this time has its figure
//! carried forward marked stale rather than blanked, and a panel read at its own
//! pace is carried forward until it is due — both done by handing the previous
//! gather back in.
//!
//! An alert is said as it happens from the same gather too: what the dashboard's list
//! carries that it did not a moment ago is what started or resolved since.
//!
//! What is newest is said from the same gather. The household's requests and what
//! is wrong are already in the snapshot, so naming the newest of each kind costs no
//! second reading, and it is said when a listener arrives and whenever it changes
//! rather than on every tick: a phone marks a tab from it, and an unchanged mark is
//! nothing to wake it for.

use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::alert::{Alert, Moment};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::changelog::Record;
use lemonfiber_core::dashboard::run::{paced, Gathered};
use lemonfiber_core::dashboard::{Panel, Snapshot};
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::news::{Newest, News};
use tokio::sync::Mutex;

use super::live::Gathers;
use super::wire::{Nature, Rendered};

/// The dashboard's gather, as the stream's source.
pub struct Dashboard {
    /// What the gather reaches the outside world through.
    ctx: Arc<Ctx>,
    /// The gather the next one replaces, where there has been one.
    last: Mutex<Option<Gathered>>,
    /// The record of releases this build carries, read once because it is compiled in.
    record: Option<Record>,
    /// What the stream last said was newest, so it says so again only on a change.
    told: Mutex<Option<Newest>>,
    /// Every alert the dashboard carried at the last gather, by what names it and which
    /// way it went, or nothing before the first gather of this run.
    alerted: Mutex<Option<BTreeSet<(String, Moment)>>>,
}

impl Dashboard {
    /// The dashboard gathered against this context.
    #[must_use]
    pub fn against(ctx: Arc<Ctx>) -> Self {
        Self {
            ctx,
            last: Mutex::new(None),
            record: Record::carried(),
            told: Mutex::new(None),
            alerted: Mutex::new(None),
        }
    }
}

#[async_trait]
impl Gathers for Dashboard {
    async fn gather(&self, joined: bool) -> Vec<Rendered> {
        gathered(self, joined).await
    }
}

/// The dashboard rendered, with the snapshot it was made from kept for the next one,
/// and what is new where a listener has just arrived or it has changed.
async fn gathered(dashboard: &Dashboard, joined: bool) -> Vec<Rendered> {
    let mut last = dashboard.last.lock().await;
    let gathered = paced(&dashboard.ctx, last.as_ref()).await;
    let snapshot = &gathered.snapshot;
    let mut said: Vec<Rendered> =
        Rendered::of(Nature::State, &Envelope::new(kind::DASHBOARD, snapshot))
            .into_iter()
            .collect();
    said.extend(newly(dashboard, snapshot, joined).await);
    said.extend(alerted(dashboard, &snapshot.alerts).await);
    *last = Some(gathered);
    said
}

/// Each alert the dashboard carries that it did not at the last gather, oldest first, as
/// the thing that happened.
///
/// Read off the dashboard's own list rather than off the run that decided it, because
/// an alert can be decided by a run other than this one — the watcher, a boot, a key
/// minted at the terminal — and every one of them lands in that list. What it carried
/// when this run began was said before anybody here was listening and is the list's to
/// show, so the first gather says none of it. Said as a record, so a client that missed
/// one while it was away is handed it when it comes back.
async fn alerted(dashboard: &Dashboard, alerts: &[Alert]) -> Vec<Rendered> {
    let carried: BTreeSet<(String, Moment)> = alerts
        .iter()
        .filter_map(|alert| Some((alert.id.clone()?, alert.moment)))
        .collect();
    let mut alerted = dashboard.alerted.lock().await;
    let said = alerted.as_ref().map_or_else(Vec::new, |before| {
        alerts
            .iter()
            .rev()
            .filter(|alert| {
                alert
                    .id
                    .as_ref()
                    .is_some_and(|id| !before.contains(&(id.clone(), alert.moment)))
            })
            .filter_map(|alert| Rendered::of(Nature::Record, &Envelope::new(kind::ALERT, alert)))
            .collect()
    });
    *alerted = Some(carried);
    said
}

/// The newest of each kind, where it is owed: to a listener that has just arrived,
/// and to everyone when any of it has changed.
///
/// Assembled by the function the read answers from, so what the stream names as
/// newest is what the read lists first.
async fn newly(dashboard: &Dashboard, snapshot: &Snapshot, joined: bool) -> Option<Rendered> {
    let household = match &snapshot.household {
        Panel::Ready(household) => Some(household),
        Panel::Unavailable { .. } => None,
    };
    let newest = News::of(
        dashboard.record.as_ref(),
        household,
        &snapshot.health.affected,
    )
    .newest();
    let mut told = dashboard.told.lock().await;
    if !joined && told.as_ref() == Some(&newest) {
        return None;
    }
    let rendered = Rendered::of(Nature::State, &Envelope::new(kind::NEWS, &newest));
    *told = Some(newest);
    rendered
}

#[cfg(test)]
mod tests;
