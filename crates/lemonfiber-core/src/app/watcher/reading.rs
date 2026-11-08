//! `lemonfiber watcher`: what the outside watcher sees, beside what each of its monitors
//! covers.

use crate::app::Ctx;
use crate::error::Problem;
use crate::model::{Heartbeat, WatchState, WatchedTarget, WatcherReport};
use crate::ports::service::{Covers, Monitor, MonitorState};

use lemonfiber_contract::capabilities::status::watch::CAPABILITY;

use super::covering::wanted;
use super::kept::{read, Kept, Made};

/// Why a watcher is answered blind where it says it does not answer and not why.
const UNSAID: &str = "it says it does not answer";

/// What the outside watcher sees, and every critical service and household entry point
/// no active monitor covers.
///
/// A watcher that does not answer, or nothing filling `status.watch`, is answered as
/// coverage nobody can see: not reachable, and everything it should cover uncovered.
///
/// # Errors
///
/// Where the stack, the record of what is installed or the record of the monitors
/// lemonfiber provisioned cannot be read, which is never answered as nothing watched.
pub(crate) async fn watcher(ctx: &Ctx) -> Result<WatcherReport, Box<Problem>> {
    let (services, installed, fillers) = crate::seed::run::read_managed(ctx)?;
    let record = read(ctx)?;
    let read_at = ctx.seconds();
    let should = wanted(ctx, &services, &installed, &fillers).await;
    let covering = should
        .iter()
        .map(|one| one.covers.clone())
        .filter(|covers| !matches!(covers, Covers::Job(_)));
    let watcher = super::filled_by(&fillers);
    let seen = match super::asked(ctx, &fillers).await {
        None => Err(watcher.as_ref().map_or_else(
            || format!("nothing fills {CAPABILITY}"),
            |plugin| format!("{plugin} cannot be asked over {CAPABILITY}"),
        )),
        Some(asked) => seen(asked.as_ref()).await,
    };
    let monitors = match seen {
        Err(why) => {
            return Ok(WatcherReport {
                watcher,
                reachable: false,
                read_at,
                unreachable_because: Some(why),
                monitors: Vec::new(),
                uncovered: covering.collect(),
            });
        }
        Ok(monitors) => monitors,
    };
    let stopped = crate::autostart::run::load(ctx).halted();
    let uncovered = covering
        .filter(|covers| !actively(&record, &monitors, covers))
        .collect();
    Ok(WatcherReport {
        watcher,
        reachable: true,
        read_at,
        unreachable_because: None,
        monitors: monitors
            .iter()
            .map(|monitor| watched(monitor, &record, stopped, read_at))
            .collect(),
        uncovered,
    })
}

/// Every monitor the watcher holds, or why it does not answer.
async fn seen(watcher: &dyn crate::ports::service::Monitoring) -> Result<Vec<Monitor>, String> {
    match watcher.reachable().await {
        Err(failure) => return Err(failure.to_string()),
        Ok(reach) if !reach.reachable => {
            return Err(reach.why.unwrap_or_else(|| UNSAID.to_owned()))
        }
        Ok(_) => {}
    }
    watcher
        .monitors()
        .await
        .map_err(|failure| failure.to_string())
}

/// Whether an active monitor lemonfiber made covers `covers`.
fn actively(record: &Kept, monitors: &[Monitor], covers: &Covers) -> bool {
    record.made.get(&covers.tag()).is_some_and(|made| {
        monitors
            .iter()
            .any(|monitor| monitor.id == made.id && monitor.active)
    })
}

/// One monitor as the read answers it, paired by the record alone.
fn watched(monitor: &Monitor, record: &Kept, stopped: bool, now: u64) -> WatchedTarget {
    let made = record.tagged(&monitor.id);
    let covers = made.and_then(|(tag, _)| Covers::read(tag));
    let made = made.map(|(_, made)| made);
    WatchedTarget {
        state: state(monitor, covers.as_ref(), made, stopped, now),
        covers,
        name: monitor.name.clone(),
        kind: monitor.kind,
        since: monitor.since,
        last_reading: monitor.last_reading,
        heartbeat: monitor.heartbeat.map(|beat| Heartbeat {
            every_s: beat.every_s,
            grace_s: beat.grace_s,
            last_check_in: beat.last_check_in,
        }),
        active: monitor.active,
        path: made.and_then(|made| made.path),
        unconfigured: made.and_then(|made| made.unconfigured.clone()),
    }
}

/// What a monitor sees: expected down where the stack it covers is stopped on purpose,
/// not reporting where it holds no reading newer than its interval and grace, and what
/// the watcher says otherwise.
///
/// A heartbeat's interval and grace are the watcher's; a check lemonfiber made is held to
/// the interval it was provisioned at, its grace one more interval. A monitor the operator
/// made is held to neither, since nothing here knows how often it checks.
fn state(
    monitor: &Monitor,
    covers: Option<&Covers>,
    made: Option<&Made>,
    stopped: bool,
    now: u64,
) -> WatchState {
    let said = match monitor.state {
        MonitorState::Up => WatchState::Up,
        MonitorState::Down => WatchState::Down,
        MonitorState::Pending => WatchState::Pending,
        MonitorState::Paused => WatchState::Paused,
    };
    if !monitor.active {
        return said;
    }
    if stopped && matches!(covers, Some(Covers::Service(_) | Covers::EntryPoint(_))) {
        return WatchState::ExpectedDown;
    }
    let allowed = monitor
        .heartbeat
        .map(|beat| beat.every_s.saturating_add(beat.grace_s))
        .or_else(|| {
            made.map(|made| {
                let every = made.asked.every_s;
                every.saturating_add(made.asked.grace_s.unwrap_or(every))
            })
        });
    let newest = monitor.last_reading.unwrap_or(monitor.since);
    match allowed {
        Some(allowed) if now.saturating_sub(newest) > allowed => WatchState::NotReporting,
        Some(_) | None => said,
    }
}

#[cfg(test)]
mod tests;
