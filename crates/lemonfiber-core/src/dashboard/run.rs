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
//! its own, and the slow ones at the pace [`crate::dashboard::pace`] sets. At that pace
//! it also passes the egress guard's forwarded port on, while a surface is open.
//!
//! A refresh loop gathers in two waves, so a slow source holds up no panel beside it:
//! what answers within [`FIRST_WAVE`] goes out at once, and a second snapshot fills in
//! the panels still waiting.

mod panels;
mod waves;

use std::collections::BTreeSet;
use std::future::Future;
use std::path::Path;
use std::pin::{pin, Pin};
use std::time::Duration;

use futures_util::future::{maybe_done, MaybeDone};

use crate::app::targets::project_directory;
use crate::app::{forwarding, Ctx};
use crate::dashboard::pace::{Paced, Readings, FIRST_WAVE, HOUSEHOLD_WITHIN, PANEL_WITHIN};
use crate::dashboard::{Hardlink, Reading, Snapshot};
use crate::error::Diagnose;
use crate::queue::run::Answered;

#[cfg(test)]
use panels::{download_rate, egress, storage};
use panels::{free, linking, observe, queues, transfers, vpn, waiting_on};
use waves::{assembled, Answers, Asked, Wave};
#[cfg(test)]
use waves::{carried_link, downloaders};

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
///
/// A one-shot read: it answers once every source has answered or run out its bound,
/// with no first wave in between.
pub async fn gather(ctx: &Ctx, previous: Option<&Snapshot>) -> Snapshot {
    paced(ctx, previous.cloned().map(Gathered::after).as_ref())
        .await
        .snapshot
}

/// Gather one snapshot, reading each slow panel only where its pace says it is due,
/// and answer once every source has answered or run out its bound.
pub async fn paced(ctx: &Ctx, previous: Option<&Gathered>) -> Gathered {
    waves(ctx, previous, |_| ()).await
}

/// Gather in two waves: what answers within [`FIRST_WAVE`] is handed to `first` as a
/// snapshot of its own, and the snapshot answered fills in the rest.
///
/// What a refresh loop calls, handing back what the last call returned. A panel that
/// is not due is carried forward from it unchanged, and one that is due is read
/// within its bound, so a source that will not answer marks its own panel and holds
/// up nothing beside it. `first` is called only where some source has not answered
/// within the first wave; each panel still waiting is carried from the last gather,
/// or `pending` where there is nothing to carry. Only the second wave records what it
/// found and tells the operator: the first is a picture, not a reading of record.
///
/// The health summary is not read from the panels but computed from what they
/// found, through the one shared computation every surface uses. So the tunnel is
/// read before the summary rather than beside it: a stack whose containers are all
/// healthy while its download client's traffic leaves outside the tunnel is the
/// case this ordering exists for, and a first wave the tunnel has not answered
/// says `unknown`.
pub async fn waves(
    ctx: &Ctx,
    previous: Option<&Gathered>,
    first: impl FnOnce(Snapshot) + Send,
) -> Gathered {
    let mut readings = previous.map(|was| was.readings.clone()).unwrap_or_default();
    let last = previous.map(|was| &was.snapshot);
    let due = Due::at(&mut readings, ctx.seams.clock.now());

    let manifest = manifest_of(ctx);
    let manifest = manifest.as_ref();
    let directory = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let project = directory.as_deref();
    let root = ctx.settings.data_root.as_deref();
    let kept = project.map(crate::app::targets::services_config_dir);

    let mut seen = pin!(maybe_done(always(observe(ctx, manifest))));
    let mut tunnel = pin!(maybe_done(when(
        due.is(Paced::Vpn),
        PANEL_WITHIN,
        vpn(ctx, manifest, project)
    )));
    let mut household = pin!(maybe_done(when(
        due.is(Paced::Household),
        HOUSEHOLD_WITHIN,
        waiting_on(ctx)
    )));
    let mut named = pin!(maybe_done(when(
        due.is(Paced::Door),
        PANEL_WITHIN,
        ctx.site.name()
    )));
    let mut moving = pin!(maybe_done(always(transfers(ctx, manifest, project, last))));
    let mut queued = pin!(maybe_done(when(
        due.is(Paced::Queues),
        PANEL_WITHIN,
        queues(ctx, manifest, project)
    )));
    let mut asked = pin!(maybe_done(always(
        crate::bandwidth::run::pausing::standing(ctx)
    )));
    let mut space = pin!(maybe_done(free_space(ctx, root, &due)));
    let mut config_space = pin!(maybe_done(config_free(ctx, root, kept.as_deref(), &due)));
    let mut linked = pin!(maybe_done(hardlink(ctx, root, &due)));
    let mut forwarded = pin!(maybe_done(passed_on(ctx, due.is(Paced::Forwarded))));

    macro_rules! every {
        () => {
            tokio::join!(
                seen.as_mut(),
                tunnel.as_mut(),
                household.as_mut(),
                named.as_mut(),
                moving.as_mut(),
                queued.as_mut(),
                asked.as_mut(),
                space.as_mut(),
                config_space.as_mut(),
                linked.as_mut(),
                forwarded.as_mut(),
            )
        };
    }
    macro_rules! answers {
        ($read:ident) => {
            Answers {
                seen: $read(seen.as_mut()),
                tunnel: $read(tunnel.as_mut()),
                household: $read(household.as_mut()),
                named: $read(named.as_mut()),
                moving: $read(moving.as_mut()),
                queued: $read(queued.as_mut()),
                asked: $read(asked.as_mut()),
                space: $read(space.as_mut()),
                config_space: $read(config_space.as_mut()),
                linked: $read(linked.as_mut()),
                forwarded: $read(forwarded.as_mut()),
            }
        };
    }

    if tokio::time::timeout(FIRST_WAVE, async { every!() })
        .await
        .is_err()
    {
        let so_far = assembled(ctx, previous, manifest, Wave::First, answers!(peek)).await;
        first(so_far.snapshot);
        every!();
    }

    let answers = answers!(taken);
    let found = assembled(ctx, previous, manifest, Wave::Second, answers).await;
    Gathered {
        snapshot: found.snapshot,
        readings,
        named: found.named,
        answers: found.answers,
    }
}

/// The manifest every stack-derived panel reads from, resolved once — or the one reason
/// each reports if it cannot be read — so a stack that cannot be read leaves every panel
/// unavailable from the one failure.
fn manifest_of(ctx: &Ctx) -> Result<lemonfiber_manifest::Manifest, String> {
    ctx.stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem().summary)
}

/// The forwarded port passed on, where it is due, within the bound a panel has.
async fn passed_on(ctx: &Ctx, due: bool) -> Asked<Option<forwarding::Pass>> {
    when(due, PANEL_WITHIN, Box::pin(forwarding::on_the_minute(ctx))).await
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

/// The data location's free space, where it is due, and not asked where no location
/// is configured, since then there is nothing to read.
async fn free_space(ctx: &Ctx, root: Option<&Path>, due: &Due) -> Asked<Reading<u64>> {
    match root {
        Some(root) => when(due.is(Paced::FreeSpace), PANEL_WITHIN, free(ctx, root)).await,
        None => Asked::NotDue,
    }
}

/// The free space where the services keep their configuration, where it is due:
/// unknown where the stack names no such place, and not asked where no data location
/// is configured.
async fn config_free(
    ctx: &Ctx,
    root: Option<&Path>,
    kept: Option<&Path>,
    due: &Due,
) -> Asked<Reading<u64>> {
    match (root, kept) {
        (None, _) => Asked::NotDue,
        (Some(_), None) => Asked::Read(Reading::Unknown),
        (Some(_), Some(kept)) => {
            when(due.is(Paced::FreeSpace), PANEL_WITHIN, free(ctx, kept)).await
        }
    }
}

/// Whether imports into the data location link, where it is due. A rehearsal never
/// probes, since the probe writes: it carries what an earlier refresh found, or says
/// it does not know.
async fn hardlink(ctx: &Ctx, root: Option<&Path>, due: &Due) -> Asked<Hardlink> {
    match root {
        Some(root) => {
            when(
                due.is(Paced::Hardlink) && !ctx.dry_run,
                PANEL_WITHIN,
                linking(ctx, root),
            )
            .await
        }
        None => Asked::NotDue,
    }
}

/// A reading made on every refresh: what it came to within a panel's bound.
async fn always<T>(reading: impl Future<Output = T>) -> Asked<T> {
    when(true, PANEL_WITHIN, reading).await
}

/// A paced reading: not asked where it is not due, and otherwise what it came to
/// within `bound`.
async fn when<T>(due: bool, bound: Duration, reading: impl Future<Output = T>) -> Asked<T> {
    if !due {
        return Asked::NotDue;
    }
    tokio::time::timeout(bound, reading)
        .await
        .map_or(Asked::Late, Asked::Read)
}

/// What a reading has come to so far: what it answered, or pending where it is still
/// being read.
fn peek<T: Clone>(reading: Pin<&mut MaybeDone<impl Future<Output = Asked<T>>>>) -> Asked<T> {
    reading
        .output_mut()
        .map_or(Asked::Pending, |answered| answered.clone())
}

/// What a finished reading came to, taken out of it.
fn taken<T>(reading: Pin<&mut MaybeDone<impl Future<Output = Asked<T>>>>) -> Asked<T> {
    reading.take_output().unwrap_or(Asked::Late)
}

#[cfg(test)]
mod tests;
