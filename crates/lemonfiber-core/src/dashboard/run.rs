//! Assembling one dashboard snapshot from what the ports can be reached for.
//!
//! The shape of the screen and the rules that keep it honest are the pure
//! [`crate::dashboard`] module; this is the driver that fills it from the live
//! stack. It never fails — a dashboard degrades rather than errors (a source that
//! cannot be reached marks its own panel and leaves the rest), so there is no
//! error channel through which a dead source could terminate the render loop.
//!
//! This gatherer fills every read-only panel: the services and their health, the
//! storage volume (free space, hardlink status, projected exhaustion), each curator's
//! queue, each download client's active transfers, and the VPN tunnel's state. The
//! panels are read at once rather than one after another, each within a bound of
//! its own, and the slow ones at the pace [`crate::dashboard::pace`] sets.

mod panels;

use std::collections::BTreeSet;
use std::future::Future;
use std::time::Duration;

use crate::app::screen::Screen;
use crate::app::targets::project_directory;
use crate::app::{conditions, outbox, Ctx};
use crate::condition::Conditions;
use crate::dashboard::pace::{Paced, Readings, HOUSEHOLD_WITHIN, PANEL_WITHIN};
use crate::dashboard::{Hardlink, Panel, Reading, Snapshot, Telemetry};
use crate::error::Diagnose;
use crate::health::Reach;
use crate::notify::run as notify;
use crate::queue::run::Answered;
use crate::queue::Thresholds;

use panels::{
    download_rate, downloading, egress, free, front_door, last_config_free, last_free, linking,
    observe, queues, storage, summarise, transfers, vpn, waiting_on,
};

/// How many alerts the screen carries. Enough to see what happened, few enough
/// that the newest is not buried under a week of history.
const SHOWN_ALERTS: usize = 8;

/// One gather, and what the next one carries forward from it.
///
/// The snapshot is what the screen shows. The rest is what a panel read at its own
/// pace needs between its readings — when each was last read, and what the two
/// readings that are not on the screen came to — so the next gather can carry them
/// forward rather than read them again.
#[derive(Debug, Clone)]
pub struct Gathered {
    /// What the screen shows.
    pub snapshot: Snapshot,
    /// When each paced panel was last read.
    readings: Readings,
    /// What this machine called itself when it was last asked.
    named: Option<String>,
    /// What each curator's queue held when it was last read.
    answers: Vec<(String, Answered)>,
}

impl Gathered {
    /// A gather carrying forward only what `snapshot` shows, with every panel due.
    fn after(snapshot: Snapshot) -> Self {
        Self {
            snapshot,
            readings: Readings::default(),
            named: None,
            answers: Vec::new(),
        }
    }

    /// The same gather with every panel due again, for a refresh somebody asked for.
    ///
    /// What the screen shows is kept, so a figure that does not answer this time is
    /// carried forward marked stale rather than blanked.
    #[must_use]
    pub fn due_now(&self) -> Self {
        Self {
            readings: Readings::default(),
            ..self.clone()
        }
    }
}

/// Gather one snapshot of what the stack is doing right now, every panel read afresh.
///
/// `previous` is the snapshot this one replaces, where there is one. A figure a
/// source gave a moment ago and did not give this time is carried forward marked
/// stale rather than blanked: the source has told us something, and throwing it
/// away is as dishonest as presenting it as current. A first refresh has nothing
/// to carry, so it passes `None`.
pub async fn gather(ctx: &Ctx, previous: Option<&Snapshot>) -> Snapshot {
    paced(ctx, previous.cloned().map(Gathered::after).as_ref())
        .await
        .snapshot
}

/// Gather one snapshot, reading each slow panel only where its pace says it is due.
///
/// What a refresh loop calls, handing back what the last call returned. A panel that
/// is not due is carried forward from it unchanged, and one that is due is read
/// within its bound — a source that will not answer marks its own panel and holds up
/// nothing beside it.
///
/// The health summary is not read from the panels but computed from what they
/// found, through the one shared computation every surface uses. So the tunnel is
/// read before the summary rather than beside it: a stack whose containers are all
/// healthy while its download client's traffic leaves outside the tunnel is the
/// case this ordering exists for.
pub async fn paced(ctx: &Ctx, previous: Option<&Gathered>) -> Gathered {
    let now = ctx.seams.clock.now();
    let mut readings = previous.map(|was| was.readings.clone()).unwrap_or_default();
    let last = previous.map(|was| &was.snapshot);
    let due = Due::at(&mut readings, now);

    let configured = ctx.settings.data_root.is_some();
    // The manifest every stack-derived panel reads from, resolved once — or the one
    // reason each reports if it cannot be read — so a stack that cannot be read
    // leaves every panel unavailable from the one failure.
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem().summary);
    let manifest = manifest.as_ref();
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let root = ctx.settings.data_root.as_deref();

    let (seen, tunnel, household, named, moving, queued, asked, (space, kept, linked)) = tokio::join!(
        within(PANEL_WITHIN, observe(ctx, manifest)),
        when(due.is(Paced::Vpn), PANEL_WITHIN, vpn(ctx, manifest)),
        when(due.is(Paced::Household), HOUSEHOLD_WITHIN, waiting_on(ctx)),
        when(due.is(Paced::Door), PANEL_WITHIN, ctx.site.name()),
        within(
            PANEL_WITHIN,
            transfers(ctx, manifest, project.as_deref(), last)
        ),
        when(
            due.is(Paced::Queues),
            PANEL_WITHIN,
            queues(ctx, manifest, project.as_deref())
        ),
        within(PANEL_WITHIN, crate::bandwidth::run::pausing::standing(ctx)),
        volume(ctx, root, project.as_deref(), &due),
    );

    let (seen, undeclared) = match seen.unwrap_or_else(|| Err(late(ENGINE))) {
        Ok((surveyed, undeclared)) => (Ok(surveyed), undeclared),
        Err(reason) => (Err(reason), Vec::new()),
    };
    let reach = Reach::of(configured, seen.as_deref().ok());
    let vpn = tunnel.settled(
        last.map(|was| was.vpn.clone()),
        Some(Panel::unavailable(late(TUNNEL))),
    );
    let household = household.settled(
        last.map(|was| was.household.clone()),
        Panel::unavailable(late_after(HOUSEHOLD, HOUSEHOLD_WITHIN)),
    );
    let named = named.settled(previous.map(|was| was.named.clone()), None);
    let transfers = moving.unwrap_or(Panel::unavailable(late(DOWNLOADS)));
    let (queue, answers) = queued.settled(
        previous.map(|was| (was.snapshot.queue.clone(), was.answers.clone())),
        (Panel::unavailable(late(QUEUES)), Vec::new()),
    );

    // One store for the whole refresh: the health summary, the queue check and the
    // notifier all read and write the same history, and three loads would be three
    // pictures of it — the last one written winning.
    let mut conditions = conditions::load(ctx);
    let found = conditions.clone();
    // A stack that could not be read has nothing to raise conditions about; the
    // summary then rests on the reach alone and says `unknown` rather than healthy.
    let health = summarise(
        ctx,
        reach,
        seen.as_deref().unwrap_or_default(),
        egress(vpn.as_ref()),
        &mut conditions,
    );
    let services = match seen {
        Ok(services) => Panel::Ready(services),
        Err(reason) => Panel::unavailable(reason),
    };
    let door = front_door(ctx, manifest, &services, &undeclared, named.as_deref());

    let storage = stored(
        root,
        (
            carried(space, last_free(last)),
            carried(kept, last_config_free(last)),
        ),
        carried_link(linked, last),
        &transfers,
    );

    // What the pipeline is doing, assessed across the services together. Recorded
    // against the same store, so a stall that has held for a day is known to have
    // held for a day rather than looking new on every refresh.
    let watched = crate::queue::run::watch(
        &answers,
        &downloading(&transfers),
        &mut conditions,
        Thresholds::conservative(),
        &ctx.stamp(),
    );

    let alerts = told(ctx, reach, &mut conditions, &found).await;

    Gathered {
        snapshot: Snapshot {
            // Not read from the panels: each one that could not be filled says so in
            // its own words, and the screen around it is still refreshing.
            telemetry: Telemetry::read(reach, false),
            health,
            vpn,
            transfers,
            queue,
            downloaders: downloaders(asked),
            stuck: watched.stuck,
            alerts,
            storage,
            services,
            door,
            household,
        },
        readings,
        named,
        answers,
    }
}

/// Which paced panels are due this refresh, each recorded as read at `now` if it is.
struct Due(BTreeSet<Paced>);

impl Due {
    /// What is due at `now`, by the readings the last refresh left.
    fn at(readings: &mut Readings, now: std::time::SystemTime) -> Self {
        let mut due = BTreeSet::new();
        for panel in Paced::ALL {
            if readings.due(panel, now) {
                readings.read(panel, now);
                due.insert(panel);
            }
        }
        Self(due)
    }

    /// Whether `panel` is read this refresh.
    fn is(&self, panel: Paced) -> bool {
        self.0.contains(&panel)
    }
}

/// Tell the operator what this refresh found, and answer with what the screen shows.
///
/// Last, because everything gathered is what there is to tell them about — and
/// through the screen, which is the one channel that needs no configuring and cannot
/// be down. The history and the outbox are written only where this refresh changed
/// them: a store rewritten every second with what it already held is a flush to the
/// disk for nothing. A rehearsal writes neither: what it found is shown, and the
/// records stay as they were for the run that is not one.
async fn told(
    ctx: &Ctx,
    reach: Reach,
    conditions: &mut Conditions,
    found: &Conditions,
) -> Vec<crate::alert::Alert> {
    let mut outbox = outbox::load(ctx);
    let owed = outbox.clone();
    notify::notify(ctx, reach, conditions, &mut outbox, &[&Screen]).await;
    let alerts = outbox
        .owing()
        .iter()
        .chain(outbox.history())
        .take(SHOWN_ALERTS)
        .cloned()
        .collect();
    if !ctx.dry_run && conditions != found {
        conditions::save(ctx, conditions);
    }
    if !ctx.dry_run && outbox != owed {
        outbox::save(ctx, &outbox);
    }
    alerts
}

/// The storage panel, where there is a data location to have one.
///
/// Its exhaustion projection needs the rate downloads are landing on the disk at, so
/// it is worked out from the transfers read this refresh, whichever free space
/// reading is current.
fn stored(
    root: Option<&std::path::Path>,
    (free, config_free): (Reading<u64>, Reading<u64>),
    hardlink: Hardlink,
    transfers: &Panel<Vec<crate::dashboard::Transfer>>,
) -> Panel<crate::dashboard::Storage> {
    match root {
        None => Panel::unavailable("no data location is configured"),
        Some(_) => Panel::Ready(storage(
            free,
            config_free,
            hardlink,
            download_rate(transfers),
        )),
    }
}

/// What a paced panel's reading came to this refresh.
#[derive(Debug, PartialEq, Eq)]
enum Asked<T> {
    /// It was not due, so what it showed last stands.
    NotDue,
    /// It was due and answered in time.
    Read(T),
    /// It was due and did not answer within its bound.
    Late,
}

impl<T> Asked<T> {
    /// What a panel shows: what it read, `late` where that reading did not answer in
    /// time, and otherwise what it showed last.
    fn settled(self, carried: Option<T>, late: T) -> T {
        match self {
            Self::Read(read) => read,
            Self::Late => late,
            Self::NotDue => carried.unwrap_or(late),
        }
    }
}

/// What a free space reads as this refresh: what was read now, carried forward
/// marked stale where it could not be, or the last reading where none was due.
fn carried(space: Asked<Reading<u64>>, last: Option<&Reading<u64>>) -> Reading<u64> {
    match space {
        Asked::NotDue => last.copied().unwrap_or(Reading::Unknown),
        asked => asked.settled(None, Reading::Unknown).or_stale(last),
    }
}

/// Whether imports link, as of this refresh: what the probe found now, or what it
/// found last where it was not due.
fn carried_link(linked: Asked<Hardlink>, last: Option<&Snapshot>) -> Hardlink {
    let shown = match last.map(|was| &was.storage) {
        Some(Panel::Ready(storage)) => Some(storage.hardlink),
        _ => None,
    };
    linked.settled(shown, Hardlink::Unknown)
}

/// The data location's free space, the free space where the services keep their
/// configuration, and whether imports into the data location link, each where it is
/// due — and none where no location is configured, since then there is nothing to
/// read. A rehearsal never probes, since the probe writes: it carries what an earlier
/// refresh found, or says it does not know.
async fn volume(
    ctx: &Ctx,
    root: Option<&std::path::Path>,
    project: Option<&std::path::Path>,
    due: &Due,
) -> (Asked<Reading<u64>>, Asked<Reading<u64>>, Asked<Hardlink>) {
    let Some(root) = root else {
        return (Asked::NotDue, Asked::NotDue, Asked::NotDue);
    };
    let kept = project.map(crate::app::targets::services_config_dir);
    tokio::join!(
        when(due.is(Paced::FreeSpace), PANEL_WITHIN, free(ctx, root)),
        async {
            match kept.as_deref() {
                Some(kept) => when(due.is(Paced::FreeSpace), PANEL_WITHIN, free(ctx, kept)).await,
                None => Asked::Read(Reading::Unknown),
            }
        },
        when(
            due.is(Paced::Hardlink) && !ctx.dry_run,
            PANEL_WITHIN,
            linking(ctx, root)
        ),
    )
}

/// What a reading came to within `bound`, or nothing where it did not answer in time.
async fn within<T>(bound: Duration, reading: impl Future<Output = T>) -> Option<T> {
    tokio::time::timeout(bound, reading).await.ok()
}

/// A paced reading: not asked where it is not due, and otherwise what it came to
/// within `bound`.
async fn when<T>(due: bool, bound: Duration, reading: impl Future<Output = T>) -> Asked<T> {
    if !due {
        return Asked::NotDue;
    }
    within(bound, reading)
        .await
        .map_or(Asked::Late, Asked::Read)
}

/// Whether each download client is paused, from what each said it is doing: unknown
/// for a client that could not be asked, and the panel unavailable where the stack
/// could not be read or the clients did not answer in time.
fn downloaders(
    asked: Option<Result<Vec<crate::bandwidth::pausing::Paused>, Box<crate::error::Problem>>>,
) -> Panel<Vec<crate::dashboard::Downloader>> {
    use crate::bandwidth::Pulling;
    use crate::dashboard::Fetching;
    match asked {
        None => Panel::unavailable(late(DOWNLOADS)),
        Some(Err(problem)) => Panel::unavailable(problem.summary),
        Some(Ok(each)) => Panel::Ready(
            each.into_iter()
                .map(|said| crate::dashboard::Downloader {
                    client: said.client,
                    state: match said.was {
                        Some(Pulling::Stopped) => Fetching::Paused,
                        Some(Pulling::Fetching) => Fetching::Fetching,
                        None => Fetching::Unknown,
                    },
                })
                .collect(),
        ),
    }
}

/// The services panel's source, as a panel that did not answer names it.
const ENGINE: &str = "the container engine";
/// The VPN panel's source, as a panel that did not answer names it.
const TUNNEL: &str = "the VPN";
/// The household panel's sources, as a panel that did not answer names them.
const HOUSEHOLD: &str = "the household's services";
/// The transfers panel's sources, as a panel that did not answer names them.
const DOWNLOADS: &str = "the download clients";
/// The queue panel's sources, as a panel that did not answer names them.
const QUEUES: &str = "the media managers";

/// Why a panel whose source did not answer within [`PANEL_WITHIN`] is unavailable.
fn late(source: &str) -> String {
    late_after(source, PANEL_WITHIN)
}

/// Why a panel whose source did not answer within `bound` is unavailable.
fn late_after(source: &str, bound: Duration) -> String {
    format!("{source} did not answer within {} seconds", bound.as_secs())
}

#[cfg(test)]
mod tests;
