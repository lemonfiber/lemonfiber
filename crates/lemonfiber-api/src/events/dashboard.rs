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
//! The gather comes in two waves where some source is slow: what the fast sources
//! answered is said as a dashboard of its own at once, and the dashboard that
//! follows fills in the rest.
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
use lemonfiber_core::dashboard::run::{waves, Gathered};
use lemonfiber_core::dashboard::{Panel, Snapshot};
use lemonfiber_core::news::{Newest, News};
use tokio::sync::Mutex;

use lemonfiber_core::app::{dispatch, Command, Polled, Surfacing};
use tokio::time::Instant;

use super::copying::Heard;
use super::live::{Early, Gathers};
use super::theirs::{due, playing_unread, PLAYING_EVERY};
use super::wire::{Audience, EventKind, Rendered};

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
    /// When what is playing was last read, and what was last said of it.
    playing: Mutex<(Option<Instant>, Option<String>)>,
    copying: Mutex<Heard>,
    /// What is being delivered to the phones that asked to be woken.
    waking: lemonfiber_core::push::run::Waking,
    /// What is being delivered to the back-ends.
    fanning: lemonfiber_core::notify::fanning::Fanning,
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
            playing: Mutex::new((None, None)),
            copying: Mutex::new(Heard::default()),
            waking: lemonfiber_core::push::run::Waking::default(),
            fanning: lemonfiber_core::notify::fanning::Fanning::default(),
        }
    }
}

#[async_trait]
impl Gathers for Dashboard {
    async fn gather(&self, joined: bool) -> Vec<Rendered> {
        gathered(self, joined, None).await
    }

    async fn waves(&self, joined: bool, early: &Early) -> Vec<Rendered> {
        gathered(self, joined, Some(early)).await
    }

    async fn wanted_unheard(&self) -> bool {
        let standing = crate::standing::standing(&self.ctx).await;
        standing.waiting || standing.told
    }
}

/// The dashboard rendered, with the snapshot it was made from kept for the next one,
/// and what is new where a listener has just arrived or it has changed.
///
/// What the fast sources answered goes to `early` as a dashboard of its own where
/// some source is slower, so the panels beside a slow one are not held up by it.
async fn gathered(dashboard: &Dashboard, joined: bool, early: Option<&Early>) -> Vec<Rendered> {
    let mut last = dashboard.last.lock().await;
    let gathered = waves(&dashboard.ctx, last.as_ref(), |first| {
        if let Some(early) = early {
            early.say(shared(&first));
        }
    })
    .await;
    let snapshot = &gathered.snapshot;
    let mut said: Vec<Rendered> = shared(snapshot).into_iter().collect();
    said.extend(newly(dashboard, snapshot, joined).await);
    said.extend(alerted(dashboard, &snapshot.alerts).await);
    dashboard.waking.owing(&dashboard.ctx, &snapshot.alerts);
    dashboard.fanning.owing(&dashboard.ctx, &snapshot.alerts);
    said.extend(playing(dashboard, joined).await);
    said.extend(copying(dashboard, joined).await);
    *last = Some(gathered);
    said
}

/// The dashboard as the shared stream says it: every run-and-exit service without the
/// lines its last run wrote, since a key and a member hear the stream too.
fn shared(snapshot: &Snapshot) -> Option<Rendered> {
    let mut said = snapshot.clone();
    said.scheduled = lemonfiber_core::scheduled::unlogged(said.scheduled);
    Rendered::of(EventKind::Dashboard, &said)
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
            .filter_map(|alert| Rendered::of(EventKind::Alert, alert))
            .collect()
    });
    *alerted = Some(carried);
    said
}

/// Every session playing, as the read answers the operator, read at a member stream's
/// pace and said to a listener that has just arrived and to everyone when it changed.
///
/// On the operator's stream as on a member's, so a program reading the stack's state
/// off the stream has no second thing to poll.
async fn playing(dashboard: &Dashboard, joined: bool) -> Option<Rendered> {
    let mut heard = dashboard.playing.lock().await;
    let now = Instant::now();
    if !joined && !due(heard.0, PLAYING_EVERY, now) {
        return None;
    }
    heard.0 = Some(now);
    let outcome = dispatch(
        Command::Surface(Surfacing::Polled(Polled::Playing { member: None })),
        &dashboard.ctx,
    )
    .await
    .unwrap_or_else(|problem| playing_unread(&problem));
    let rendered = Rendered::of(EventKind::Playing, &outcome)
        .filter(|_| outcome.kind() == EventKind::Playing.kind())?;
    let previous = heard.1.replace(rendered.said().to_owned());
    (joined || previous.as_deref() != Some(rendered.said())).then_some(rendered)
}

/// The copy running now, where it is owed: in full to the operator, and to a `read` or an
/// `act` key as the read answers a key.
async fn copying(dashboard: &Dashboard, joined: bool) -> Vec<Rendered> {
    let answered = dispatch(
        Command::Surface(Surfacing::Polled(Polled::Copying)),
        &dashboard.ctx,
    )
    .await;
    let said = dashboard
        .copying
        .lock()
        .await
        .heard(answered, joined, Instant::now());
    said.into_iter()
        .flat_map(|said| {
            let keys = said.clone().for_a_key();
            [(said, Audience::Operator), (keys, Audience::Key)]
        })
        .filter_map(|(said, to)| Rendered::of(EventKind::Copying, &said).map(|said| said.to(to)))
        .collect()
}

/// The newest of each kind, where it is owed: to a listener that has just arrived,
/// and to everyone when any of it has changed.
///
/// Assembled by the function the read answers from, so what the stream names as
/// newest is what the read lists first.
async fn newly(dashboard: &Dashboard, snapshot: &Snapshot, joined: bool) -> Option<Rendered> {
    let household = match &snapshot.household {
        Panel::Ready(household) => Some(household),
        Panel::Unavailable { .. } | Panel::Pending => None,
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
    let rendered = Rendered::of(EventKind::News, &newest);
    *told = Some(newest);
    rendered
}

#[cfg(test)]
mod tests;
