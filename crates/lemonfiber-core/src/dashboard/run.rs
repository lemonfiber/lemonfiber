//! Assembling one dashboard snapshot from what the ports can be reached for.
//!
//! The shape of the screen and the rules that keep it honest are the pure
//! [`crate::dashboard`] module; this is the driver that fills it from the live
//! stack. It never fails — a dashboard degrades rather than errors (a source that
//! cannot be reached marks its own panel and leaves the rest), so there is no
//! error channel through which a dead source could terminate the render loop.
//!
//! This gatherer fills every read-only panel: the services and their health, the
//! storage volume (free space, hardlink status, projected exhaustion), each \*arr's
//! queue, each download client's active transfers, and the VPN tunnel's state. What
//! remains is the ratatui surface that renders all of it on a refresh loop.

use std::path::Path;

use lemonfiber_manifest::Manifest;

use crate::app::Ctx;
use crate::dashboard::{
    eta, Hardlink, Panel, Queue, Reading, Snapshot, Storage, Telemetry, Transfer, Vpn,
};
use crate::docker::{survey, Service};
use crate::doctor::vpn::{read_vpn, VpnReading};
use crate::error::Diagnose;
use crate::health::{observed, Egress, Reach, Summary};
use crate::model::FrontDoorReport;

use crate::app::screen::Screen;
use crate::app::{conditions, outbox};
use crate::condition::Conditions;
use crate::notify::run as notify;
use crate::ports::service::{QueueDepth, Queues};
use crate::queue::run::Answered;
use crate::queue::Thresholds;
use crate::storage::{test_link, Linked};

use crate::app::targets::{
    download_targets, project_directory, protocol_of, read_transfers, servarr_targets,
};

/// Gather one snapshot of what the stack is doing right now.
///
/// Reads the services through the engine; a stack that cannot be reached leaves
/// their panel unavailable and the telemetry disconnected. The panels without a
/// gatherer are marked pending — they do not drag the telemetry down, because a
/// panel nobody has wired is not a source that failed.
///
/// The health summary is not read from the panels but computed from what they
/// found, through the one shared computation every surface uses. So the tunnel is
/// read before the summary rather than beside it: a stack whose containers are all
/// healthy while its download client's traffic leaves outside the tunnel is the
/// case this ordering exists for.
///
/// `previous` is the snapshot this one replaces, where there is one. A figure a
/// source gave a moment ago and did not give this time is carried forward marked
/// stale rather than blanked: the source has told us something, and throwing it
/// away is as dishonest as presenting it as current. A first refresh has nothing
/// to carry, so it passes `None`.
pub async fn gather(ctx: &Ctx, previous: Option<&Snapshot>) -> Snapshot {
    let configured = ctx.settings.data_root.is_some();
    // The manifest every stack-derived panel reads from, resolved once — or the one
    // reason each reports if it cannot be read. Read once rather than re-parsed and
    // re-validated by each panel a second apart on the refresh loop, and so a stack
    // that cannot be read leaves every panel unavailable from the one failure.
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem().summary);
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());

    // One store for the whole refresh: the health summary, the queue check and the
    // notifier all read and write the same history, and three loads would be three
    // pictures of it — the last one written winning.
    let mut conditions = conditions::load(ctx);
    let seen = observe(ctx, manifest.as_ref()).await;
    let reach = Reach::of(configured, seen.as_deref().ok());
    let vpn = vpn(ctx, manifest.as_ref()).await;

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
    let door = front_door(ctx, manifest.as_ref(), &services).await;
    let household = waiting_on(ctx).await;

    // Storage's exhaustion projection needs the rate downloads are landing on the
    // disk at, so the transfers are gathered first and their speeds carried into it.
    let transfers = transfers(ctx, manifest.as_ref(), project.as_deref(), previous).await;
    let storage = storage(ctx, download_rate(&transfers), previous).await;

    let (queue, answers) = queues(ctx, manifest.as_ref(), project.as_deref()).await;

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

    // And then the operator is told. Last, because everything above is what there
    // is to tell them about — and through the screen, which is the one channel
    // that needs no configuring and cannot be down.
    let mut outbox = outbox::load(ctx);
    notify::notify(ctx, reach, &mut conditions, &mut outbox, &[&Screen]).await;
    let alerts = outbox
        .owing()
        .iter()
        .chain(outbox.history())
        .take(SHOWN_ALERTS)
        .cloned()
        .collect();
    conditions::save(ctx, &conditions);
    outbox::save(ctx, &outbox);

    Snapshot {
        // Only a source that genuinely failed marks the screen degraded; the
        // pending panels below are not-yet-built, not down, so they pass `false`.
        telemetry: Telemetry::read(reach, false),
        health,
        vpn,
        transfers,
        queue,
        stuck: watched.stuck,
        alerts,
        storage,
        services,
        door,
        household,
    }
}

/// The one address to hand somebody who lives here, from the reading the panels
/// beside it are built from.
///
/// Assembled here rather than left to whichever surface draws it, so the screen and
/// `lemonfiber front-door` cannot come to name different doors — and asked afresh on
/// every gather, because a machine renamed since the last one answers as it is now
/// and there is nothing remembered to go stale.
///
/// A panel rather than a value, because it has the same two sources every other
/// panel has: a stack that will not read and an engine that will not answer both
/// leave it unfillable, and each says which in its own words.
/// What the household has asked for, for the panel beside the door.
///
/// The same reading `household` answers with, so the screen and the question cannot
/// report different requests. A failure is carried as an unavailable panel rather
/// than emptying the screen: a request service that is down is a thing to say, not a
/// household that has asked for nothing.
async fn waiting_on(ctx: &Ctx) -> Panel<crate::model::HouseholdReport> {
    match crate::household::run::household(ctx, None).await {
        Ok(report) => Panel::Ready(report),
        Err(problem) => Panel::unavailable(problem.summary.clone()),
    }
}

async fn front_door(
    ctx: &Ctx,
    manifest: Result<&Manifest, &String>,
    services: &Panel<Vec<Service>>,
) -> Panel<FrontDoorReport> {
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(reason) => return Panel::unavailable(reason.clone()),
    };
    let running = match services {
        Panel::Ready(running) => running,
        Panel::Unavailable { reason } => return Panel::unavailable(reason.clone()),
    };
    Panel::Ready(crate::door::run::assembled(
        &manifest.services,
        running,
        ctx.site.name().await.as_deref(),
        ctx.settings.household_host.as_deref(),
        ctx.settings.front_door.as_deref(),
        ctx.environment,
    ))
}

/// How many alerts the screen carries. Enough to see what happened, few enough
/// that the newest is not buried under a week of history.
const SHOWN_ALERTS: usize = 8;

/// What the download clients are moving, in the shape the queue check reads:
/// what it is called, how far along, and whether it is going anywhere.
///
/// A speed nobody could read counts as moving: not knowing is not evidence of a
/// stall, and calling it one would raise a fault about a reading rather than
/// about a download.
fn downloading(transfers: &Panel<Vec<Transfer>>) -> Vec<(String, u8, bool)> {
    match transfers {
        Panel::Ready(transfers) => transfers
            .iter()
            .map(|transfer| {
                let moving = transfer.speed.value().is_none_or(|speed| *speed > 0);
                (transfer.name.clone(), transfer.progress, moving)
            })
            .collect(),
        Panel::Unavailable { .. } => Vec::new(),
    }
}

/// Record what this refresh saw against the store the last one left, and
/// summarise from it.
///
/// Through the store rather than straight from the observations, because how long
/// a fault has lasted is the difference between a service that restarted once and
/// one that has been down all morning — and the summary grades them differently.
/// Read and written each refresh: the file is small, and a refresh that is not
/// remembered is one the next one has to guess about.
fn summarise(
    ctx: &Ctx,
    reach: Reach,
    services: &[Service],
    egress: Egress,
    conditions: &mut Conditions,
) -> Summary {
    let now = ctx.stamp();
    for (check, fault) in observed(services, egress) {
        conditions.observe(&check, fault.as_ref(), &now);
    }
    // Everything the store knows, not only what is raised: a fault that has been
    // flapping is not called fixed the moment it blinks off.
    Summary::of(reach, &conditions.all(), &now)
}

/// What one download's speed was last time it was seen, where it was.
///
/// Matched by name, since that is what identifies the same download across
/// refreshes; a download that has only just appeared has nothing to carry.
fn last_speed<'a>(previous: Option<&'a Snapshot>, name: &str) -> Option<&'a Reading<u64>> {
    match previous.map(|snapshot| &snapshot.transfers) {
        Some(Panel::Ready(active)) => active
            .iter()
            .find(|transfer| transfer.name == name)
            .map(|transfer| &transfer.speed),
        _ => None,
    }
}

/// What the volume's free space last read as, where it read at all.
fn last_free(previous: Option<&Snapshot>) -> Option<&Reading<u64>> {
    match previous.map(|snapshot| &snapshot.storage) {
        Some(Panel::Ready(storage)) => Some(&storage.free),
        _ => None,
    }
}

/// What the VPN panel proved about the download client's traffic.
///
/// A panel that could not be filled is unreadable rather than fine: the reason to
/// run a torrent client behind a tunnel is unverified, and the summary is entitled
/// to say so.
fn egress(vpn: Option<&Panel<Vpn>>) -> Egress {
    match vpn {
        None => Egress::NotApplicable,
        Some(Panel::Unavailable { .. }) => Egress::Unreadable,
        Some(Panel::Ready(vpn)) if vpn.egress_matches => Egress::Behind,
        Some(Panel::Ready(_)) => Egress::Leaking,
    }
}

/// The combined rate the active downloads are landing on the disk at — the sum of
/// the speeds actually reported this refresh, in bytes per second. A source that
/// went quiet contributes nothing rather than a guess, so a stalled queue projects
/// no exhaustion rather than a false one.
fn download_rate(transfers: &Panel<Vec<Transfer>>) -> u64 {
    match transfers {
        Panel::Ready(active) => active
            .iter()
            .filter_map(|transfer| match transfer.speed {
                Reading::Known(bytes) => Some(bytes),
                Reading::Stale(_) | Reading::Unknown => None,
            })
            .sum(),
        Panel::Unavailable { .. } => 0,
    }
}

/// The active downloads across the stack's download clients.
///
/// Resolves the download clients to host-side targets, then reads each on its own
/// shape — qBittorrent authenticated with the recorded password, `SABnzbd` with the
/// key it wrote to disk. A client not yet seeded (no password, or no key on disk)
/// or one that will not answer is left out rather than failing the panel; only a
/// stack that cannot be read at all leaves the whole panel unavailable, since then
/// there is nothing to ask. The protocol is set from which client answered, not
/// trusted from the answer.
async fn transfers(
    ctx: &Ctx,
    manifest: Result<&Manifest, &String>,
    project: Option<&Path>,
    previous: Option<&Snapshot>,
) -> Panel<Vec<Transfer>> {
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(reason) => return Panel::unavailable(reason.clone()),
    };
    let targets = download_targets(&manifest.services, project);

    // Read at once rather than in series, for the reason `doctor` runs its checks
    // that way: these are independent HTTP calls to different services, so a refresh
    // costs the slowest client rather than the sum of them — and this one happens
    // every second, where a diagnosis happens when it is asked for. `join_all` keeps
    // the order, so the panel reads the same as when they were read one at a time.
    let read = futures_util::future::join_all(targets.iter().map(|target| async move {
        (protocol_of(&target.kind), read_transfers(ctx, target).await)
    }))
    .await;

    let mut active = Vec::new();
    for (protocol, downloads) in read {
        active.extend(downloads.into_iter().map(|download| {
            Transfer {
                name: download.name.clone(),
                protocol,
                progress: download.progress,
                // A speed the client reported this refresh is known even at zero (a
                // stall); one it did not report falls back to what the same download
                // last reported, marked stale, and is unknown only where there is no
                // such thing to fall back to.
                speed: download
                    .speed
                    .map_or(Reading::Unknown, Reading::Known)
                    .or_stale(last_speed(previous, &download.name)),
                eta: download.eta,
            }
        }));
    }
    Panel::Ready(active)
}

/// What the VPN is doing, and whether the download client is genuinely behind it.
///
/// `None` where the stack has no VPN-contained torrent client — the panel does not
/// apply, rather than showing an empty box. Otherwise the tunnel's exit address,
/// country and forwarded port, and the egress-match that proves the client's
/// traffic leaves through it, or the reason none of that could be read. Reuses the
/// leak check's own containers and exec-reads ([`read_vpn`]) so the panel and the
/// diagnostic cannot disagree about what the tunnel is doing.
async fn vpn(ctx: &Ctx, manifest: Result<&Manifest, &String>) -> Option<Panel<Vpn>> {
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(reason) => return Some(Panel::unavailable(reason.clone())),
    };
    let reading = read_vpn(
        ctx.engine.as_ref(),
        &ctx.settings.project,
        manifest,
        ctx.settings.protocols,
        ctx.settings.ip_echo.clone(),
        ctx.settings.port_forward.enabled,
    )
    .await;
    match reading {
        VpnReading::NotApplicable => None,
        VpnReading::Unavailable(reason) => Some(Panel::unavailable(reason)),
        VpnReading::Ready {
            exit_ip,
            country,
            forwarded_port,
            egress_matches,
        } => Some(Panel::Ready(Vpn {
            exit_ip,
            country: country.unwrap_or_default(),
            forwarded_port,
            egress_matches,
        })),
    }
}

/// Each media-filing \*arr's queue depth and stuck count.
///
/// Resolves the Servarr-shape services the same way the credentials check does —
/// the stack's own bind-mount convention — then reads each one's key from disk and
/// asks it for its queue. A service still starting (no key written yet) or one
/// that will not answer is left out of the panel rather than failing it; only a
/// stack that cannot be read at all leaves the whole panel unavailable, since then
/// there are no services to ask.
async fn queues(
    ctx: &Ctx,
    manifest: Result<&Manifest, &String>,
    project: Option<&Path>,
) -> (Panel<Vec<Queue>>, Vec<(String, Answered)>) {
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(reason) => return (Panel::unavailable(reason.clone()), Vec::new()),
    };
    let targets = servarr_targets(&manifest.services, project);

    // Opened and read at once rather than one service after another: each is two
    // round trips, and five \*arrs in series is ten waits where the slowest one would
    // do. `join_all` keeps the order, so the panel and the answers below read the
    // same as when they were gathered one at a time.
    let read = futures_util::future::join_all(targets.iter().map(|target| async move {
        let service = target.open(&ctx.http, ctx.filesystem.as_ref()).await?;
        Some((target.name.clone(), service.queue().await))
    }))
    .await;

    let mut depths = Vec::new();
    // The items as well as the depths. A number says how much is queued and
    // cannot say which of it is stuck or why, which is what the queue check is
    // for — and asking each service twice for the same page would be a second
    // round of requests for an answer already in hand.
    let mut answers = Vec::new();
    for (name, answered) in read.into_iter().flatten() {
        match answered {
            Ok(read) => {
                let depth = QueueDepth::of(&read);
                depths.push(Queue {
                    service: name.clone(),
                    depth: depth.total,
                    stuck: depth.stuck,
                });
                answers.push((name, Answered::Queue(read.items)));
            }
            // Silence is not an empty queue, and the check is told so rather than
            // left to infer health from an absence.
            Err(_) => answers.push((name, Answered::Unreachable)),
        }
    }
    (Panel::Ready(depths), answers)
}

/// The storage picture: how much is free, whether imports link, and when it fills.
///
/// Free space is read afresh each refresh — a cheap read. A volume that could not
/// be attributed to any mount reports a zero total, and its free space is then
/// unknown rather than zero: "cannot read the volume" and "the disk is full" are
/// opposite things to an operator and must not render alike. The hardlink status
/// comes from the empirical probe, and the exhaustion from the free space against
/// the rate downloads are landing at (`download_rate`), so a stalled queue
/// projects no exhaustion rather than one that never arrives.
async fn storage(ctx: &Ctx, download_rate: u64, previous: Option<&Snapshot>) -> Panel<Storage> {
    let Some(root) = ctx.settings.data_root.as_deref() else {
        return Panel::unavailable("no data location is configured");
    };
    let facts = ctx.filesystem.describe(root).await;
    let free = if facts.total == 0 {
        Reading::Unknown
    } else {
        Reading::Known(facts.available)
    }
    .or_stale(last_free(previous));
    // The hardlink test writes — it creates a file, links it, and inspects the two
    // names — unlike the cheap free-space read. Cheap once, but a per-refresh write;
    // the refresh loop will run it far less often, and until then it runs each time.
    let hardlink = hardlink_of(&test_link(ctx.filesystem.as_ref(), root).await);
    // Exhaustion is the free space divided by the rate it is draining at: a rate of
    // zero divides to no estimate rather than an infinite one, and a volume that
    // could not be read projects nothing rather than a wrong time.
    let exhaustion = match free {
        Reading::Known(bytes) => eta(bytes, download_rate),
        Reading::Stale(_) | Reading::Unknown => None,
    };
    Panel::Ready(Storage {
        free,
        exhaustion,
        hardlink,
    })
}

/// The dashboard's hardlink status from the empirical probe: it links, it copies,
/// or it could not be established — an unwritable location or an unconfirmed link
/// is never reported as a met guarantee.
fn hardlink_of(linked: &Linked) -> Hardlink {
    match linked {
        Linked::Yes { .. } => Hardlink::Linking,
        Linked::No => Hardlink::Copying,
        Linked::Unwritable { .. } | Linked::Unconfirmed => Hardlink::Unknown,
    }
}

/// Observe every service the stack declares, or the reason it could not be read.
///
/// The reason is the operator-facing summary of whatever went wrong — an
/// unreadable stack, an engine that would not answer — so the panel that carries
/// it says something an operator can act on rather than a bare failure.
async fn observe(ctx: &Ctx, manifest: Result<&Manifest, &String>) -> Result<Vec<Service>, String> {
    let manifest = manifest.map_err(Clone::clone)?;
    let profiles: Vec<String> = manifest
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();
    let containers = ctx
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| err.problem().summary)?;
    Ok(survey(
        manifest,
        &profiles,
        &containers,
        ctx.settings.protocols,
    ))
}

#[cfg(test)]
mod tests;
