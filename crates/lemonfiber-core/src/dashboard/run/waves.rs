//! Turning what the sources answered into a snapshot, in either wave.
//!
//! One assembly for both, so the first wave and the second cannot grade the same
//! answers differently. They differ in two things only: what a panel whose source is
//! still being read shows, which is the last gather's panel or `pending` in the first
//! and its source's bound run out in the second, and whether what was found is
//! recorded and told, which only the second does.

use std::path::Path;
use std::time::Duration;

use crate::app::screen::Screen;
use crate::app::{conditions, forwarding, outbox, Ctx};
use crate::condition::Conditions;
use crate::dashboard::pace::{HOUSEHOLD_WITHIN, PANEL_WITHIN};
use crate::dashboard::{Hardlink, Panel, Reading, Snapshot, Telemetry, Transfer};
use crate::docker::{Service, Undeclared};
use crate::health::Reach;
use crate::notify::run as notify;
use crate::queue::run::Answered;
use crate::queue::Thresholds;

use super::panels::{
    download_rate, downloading, egress, front_door, last_config_free, last_free, storage, summarise,
};
use super::Gathered;

/// How many alerts the screen carries. Enough to see what happened, few enough
/// that the newest is not buried under a week of history.
const SHOWN_ALERTS: usize = 8;

/// Which of a gather's two snapshots is being assembled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Wave {
    /// What answered within the first wave, shown at once and recorded nowhere.
    First,
    /// Everything, once every source answered or ran out its bound.
    Second,
}

/// What a reading came to this refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asked<T> {
    /// It was not due, so what it showed last stands.
    NotDue,
    /// It was due and answered in time.
    Read(T),
    /// It was due and did not answer within its bound.
    Late,
    /// It is still being read, which only the first wave sees.
    Pending,
}

impl<T> Asked<T> {
    /// What a panel shows: what it read, `late` where that reading did not answer in
    /// time, what it showed last where it was not due or is still being read, and
    /// otherwise `late` where it was not due and `pending` where it is being read.
    fn settled(self, carried: Option<T>, late: T, pending: T) -> T {
        match self {
            Self::Read(read) => read,
            Self::Late => late,
            Self::NotDue => carried.unwrap_or(late),
            Self::Pending => carried.unwrap_or(pending),
        }
    }
}

/// What every source answered, or has answered so far.
pub(super) struct Answers {
    /// The services the engine reports, and the containers the stack does not declare.
    pub(super) seen: Asked<Seen>,
    /// The tunnel, or nothing where the stack has none.
    pub(super) tunnel: Asked<Option<Panel<crate::dashboard::Vpn>>>,
    /// The household's requests and standing.
    pub(super) household: Asked<Panel<crate::model::HouseholdReport>>,
    /// What this machine calls itself.
    pub(super) named: Asked<Option<String>>,
    /// The active transfers.
    pub(super) moving: Asked<Panel<Vec<Transfer>>>,
    /// Each curator's queue, and what each answered.
    pub(super) queued: Asked<Queued>,
    /// Whether each download client is paused.
    pub(super) asked: Asked<Pausing>,
    /// The data location's free space.
    pub(super) space: Asked<Reading<u64>>,
    /// The free space where the services keep their configuration.
    pub(super) config_space: Asked<Reading<u64>>,
    /// Whether imports link.
    pub(super) linked: Asked<Hardlink>,
    /// The forwarded port, passed on.
    pub(super) forwarded: Asked<Option<forwarding::Pass>>,
}

/// What the engine said of the services, and the containers the stack does not declare.
type Seen = Result<(Vec<Service>, Vec<Undeclared>), String>;

/// Each curator's queue, and what each answered.
type Queued = (Panel<Vec<crate::dashboard::Queue>>, Vec<(String, Answered)>);

/// What the download clients said of whether each is paused.
type Pausing = Result<Vec<crate::bandwidth::pausing::Paused>, Box<crate::error::Problem>>;

/// One wave's snapshot, and what the next gather carries forward from it.
pub(super) struct Assembled {
    /// What the screen shows.
    pub(super) snapshot: Snapshot,
    /// What this machine called itself.
    pub(super) named: Option<String>,
    /// What each curator's queue held.
    pub(super) answers: Vec<(String, Answered)>,
}

/// One wave's snapshot of what the sources answered.
pub(super) async fn assembled(
    ctx: &Ctx,
    previous: Option<&Gathered>,
    manifest: Result<&lemonfiber_manifest::Manifest, &String>,
    wave: Wave,
    answers: Answers,
) -> Assembled {
    let last = previous.map(|was| &was.snapshot);
    let configured = ctx.settings.data_root.is_some();
    let root = ctx.settings.data_root.as_deref();

    let (services, undeclared) = engine(answers.seen, last);
    let surveyed = match &services {
        Panel::Ready(surveyed) => Some(surveyed.as_slice()),
        Panel::Unavailable { .. } | Panel::Pending => None,
    };
    let reach = Reach::of(configured, surveyed);
    let vpn = answers.tunnel.settled(
        last.map(|was| was.vpn.clone()),
        Some(Panel::unavailable(late(TUNNEL))),
        Some(Panel::Pending),
    );
    let household = answers.household.settled(
        last.map(|was| was.household.clone()),
        Panel::unavailable(late_after(HOUSEHOLD, HOUSEHOLD_WITHIN)),
        Panel::Pending,
    );
    let named = answers
        .named
        .settled(previous.map(|was| was.named.clone()), None, None);
    let transfers = answers.moving.settled(
        last.map(|was| stale(&was.transfers)),
        Panel::unavailable(late(DOWNLOADS)),
        Panel::Pending,
    );
    let (queue, queued) = answers.queued.settled(
        previous.map(|was| (was.snapshot.queue.clone(), was.answers.clone())),
        (Panel::unavailable(late(QUEUES)), Vec::new()),
        (Panel::Pending, Vec::new()),
    );

    // One store for the whole refresh: the health summary, the queue check and the
    // notifier all read and write the same history, and three loads would be three
    // pictures of it — the last one written winning.
    let mut conditions = conditions::load(ctx);
    let found = conditions.clone();
    if wave == Wave::Second {
        observe_passed(ctx, &mut conditions, &answers.forwarded);
    }
    // A stack that could not be read has nothing to raise conditions about; the
    // summary then rests on the reach alone and says `unknown` rather than healthy.
    let health = summarise(
        ctx,
        reach,
        surveyed.unwrap_or_default(),
        egress(vpn.as_ref()),
        &mut conditions,
    );
    let door = match (&services, last) {
        (Panel::Pending, Some(was)) => was.door.clone(),
        _ => front_door(ctx, manifest, &services, &undeclared, named.as_deref()),
    };

    let storage = if answers.space == Asked::Pending
        && root.is_some()
        && !last.is_some_and(|was| was.storage.is_available())
    {
        Panel::Pending
    } else {
        stored(
            root,
            (
                carried(answers.space, last_free(last)),
                carried(answers.config_space, last_config_free(last)),
            ),
            carried_link(answers.linked, last),
            &transfers,
        )
    };

    // What the pipeline is doing, assessed across the services together. Recorded
    // against the same store, so a stall that has held for a day is known to have
    // held for a day rather than looking new on every refresh.
    let watched = crate::queue::run::watch(
        &queued,
        &downloading(&transfers),
        &mut conditions,
        Thresholds::conservative(),
        &ctx.stamp(),
    );

    let alerts = match wave {
        Wave::Second => told(ctx, reach, &mut conditions, &found).await,
        Wave::First => shown(&outbox::load(ctx)),
    };

    Assembled {
        snapshot: Snapshot {
            telemetry: telemetry(reach, configured, &services),
            health,
            vpn,
            transfers,
            queue,
            downloaders: downloaders(answers.asked, last),
            stuck: watched.stuck,
            alerts,
            storage,
            services,
            door,
            household,
        },
        named,
        answers: queued,
    }
}

/// The services panel, and the containers the stack does not declare: what the
/// engine answered, the last gather's panel where it is still being asked, and
/// otherwise why it could not be read.
fn engine(seen: Asked<Seen>, last: Option<&Snapshot>) -> (Panel<Vec<Service>>, Vec<Undeclared>) {
    match seen {
        Asked::Read(Ok((surveyed, undeclared))) => (Panel::Ready(surveyed), undeclared),
        Asked::Read(Err(reason)) => (Panel::unavailable(reason), Vec::new()),
        Asked::Pending => (
            last.map_or(Panel::Pending, |was| was.services.clone()),
            Vec::new(),
        ),
        Asked::NotDue | Asked::Late => (Panel::unavailable(late(ENGINE)), Vec::new()),
    }
}

/// How the screen is doing.
///
/// Not read from the panels: each one that could not be filled says so in its own
/// words, and the screen around it is still refreshing. An engine still being asked
/// in the first wave is not one that could not be reached, so a configured stack
/// whose engine has not answered yet reads live.
fn telemetry(reach: Reach, configured: bool, services: &Panel<Vec<Service>>) -> Telemetry {
    match services {
        Panel::Pending if configured => Telemetry::Live,
        _ => Telemetry::read(reach, false),
    }
}

/// The last gather's transfers, every speed in them marked stale: what a client said
/// a refresh ago is not what it is saying now.
fn stale(transfers: &Panel<Vec<Transfer>>) -> Panel<Vec<Transfer>> {
    match transfers {
        Panel::Ready(active) => Panel::Ready(
            active
                .iter()
                .map(|transfer| Transfer {
                    speed: Reading::Unknown.or_stale(Some(&transfer.speed)),
                    ..transfer.clone()
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Raise or clear the port not passed, where it was passed on this refresh.
fn observe_passed(
    ctx: &Ctx,
    conditions: &mut Conditions,
    passed: &Asked<Option<forwarding::Pass>>,
) {
    if let Asked::Read(Some(pass)) = passed {
        forwarding::observe(conditions, pass, &ctx.stamp());
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
    let alerts = shown(&outbox);
    if !ctx.dry_run && conditions != found {
        conditions::save(ctx, conditions);
    }
    if !ctx.dry_run && outbox != owed {
        outbox::save(ctx, &outbox);
    }
    alerts
}

/// What the screen shows of what the operator has been told: what is owed them, then
/// what has already been said.
fn shown(outbox: &crate::alert::Outbox) -> Vec<crate::alert::Alert> {
    outbox
        .owing()
        .iter()
        .chain(outbox.history())
        .take(SHOWN_ALERTS)
        .cloned()
        .collect()
}

/// The storage panel, where there is a data location to have one.
///
/// Its exhaustion projection needs the rate downloads are landing on the disk at, so
/// it is worked out from the transfers read this refresh, whichever free space
/// reading is current.
fn stored(
    root: Option<&Path>,
    (free, config_free): (Reading<u64>, Reading<u64>),
    hardlink: Hardlink,
    transfers: &Panel<Vec<Transfer>>,
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

/// What a free space reads as this refresh: what was read now, carried forward
/// marked stale where it could not be, or the last reading where none was due or it
/// is still being read.
fn carried(space: Asked<Reading<u64>>, last: Option<&Reading<u64>>) -> Reading<u64> {
    match space {
        Asked::NotDue | Asked::Pending => last.copied().unwrap_or(Reading::Unknown),
        asked => asked
            .settled(None, Reading::Unknown, Reading::Unknown)
            .or_stale(last),
    }
}

/// Whether imports link, as of this refresh: what the probe found now, or what it
/// found last where it was not due or is still probing.
pub(super) fn carried_link(linked: Asked<Hardlink>, last: Option<&Snapshot>) -> Hardlink {
    let shown = match last.map(|was| &was.storage) {
        Some(Panel::Ready(storage)) => Some(storage.hardlink),
        _ => None,
    };
    linked.settled(shown, Hardlink::Unknown, Hardlink::Unknown)
}

/// Whether each download client is paused, from what each said it is doing: unknown
/// for a client that could not be asked, the panel unavailable where the stack could
/// not be read or the clients did not answer in time, and the last gather's panel
/// where they are still being asked.
pub(super) fn downloaders(
    asked: Asked<Pausing>,
    last: Option<&Snapshot>,
) -> Panel<Vec<crate::dashboard::Downloader>> {
    use crate::bandwidth::Pulling;
    use crate::dashboard::Fetching;
    match asked {
        Asked::NotDue | Asked::Late => Panel::unavailable(late(DOWNLOADS)),
        Asked::Pending => last.map_or(Panel::Pending, |was| was.downloaders.clone()),
        Asked::Read(Err(problem)) => Panel::unavailable(problem.summary),
        Asked::Read(Ok(each)) => Panel::Ready(
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
