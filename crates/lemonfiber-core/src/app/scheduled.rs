//! What the host's scheduler calls every minute for each run-and-exit service: run it
//! where one of its times fell due or a run is owed, or at once where the operator asks,
//! and say how the run ended.

use crate::condition::Fault;
use crate::error::codes::host::NOT_SCHEDULED;
use crate::error::{Code, Diagnose, Problem, Remedy, State};
use crate::instant::{Instant, MINUTE};
use crate::model::RunReport;
use crate::ports::docker::Lifecycle;
use crate::scheduled::fires::{fired, minute_of, zone, Fired};
use crate::scheduled::kept::{self, Kept, ScheduledRun};
use crate::scheduled::Scheduled;
use crate::stack::compose::Action;

use super::Ctx;

/// Every code a run of a service on its schedule is refused with before it runs.
pub const REFUSALS: [Code; 1] = [NOT_SCHEDULED];

/// Why nothing was looked at while another run of the service holds its lock.
const RUNNING: &str = "a run of it is still going, which accounts for this minute when it ends";

/// Why nothing was looked at while the stack is down.
const DOWN: &str = "the stack is not running, so it runs once when the stack next starts";

/// Why nothing was looked at where its setting names no time.
const TIMELESS: &str = "its schedule setting names no time it runs at";

/// Look at `service` for this minute, and run it where it is due, or at once where `now`.
///
/// # Errors
///
/// Where nothing is configured to keep its record in, the register cannot be read,
/// `service` is no installed run-and-exit service, or its lock cannot be opened.
pub(crate) async fn run(ctx: &Ctx, service: &str, now: bool) -> Result<RunReport, Box<Problem>> {
    let directory =
        records(ctx).ok_or_else(|| Box::new(crate::config::store::Failure::Nowhere.problem()))?;
    let register = super::plugins::read(ctx)?;
    let Some(scheduled) = crate::scheduled::named(register.installed(), service) else {
        return Err(Box::new(not_scheduled(service)));
    };
    let held = |why: &str| Ok(report(service, ctx.dry_run, false, Some(why)));
    let lock = if ctx.dry_run {
        None
    } else {
        let Some(lock) = kept::locked(&directory, service).map_err(|why| {
            Box::new(
                crate::config::store::Failure::NotWritten {
                    path: directory.clone(),
                    reason: why.to_string(),
                }
                .problem(),
            )
        })?
        else {
            return held(RUNNING);
        };
        Some(lock)
    };
    if !up(ctx).await {
        return held(DOWN);
    }
    let holding = crate::settings::holding(ctx);
    let times = scheduled.times(&holding);
    let timeless = times
        .is_none()
        .then(|| crate::scheduled::fault(&crate::scheduled::timeless(&scheduled), None));
    if timeless.is_some() && !now {
        observed(ctx, &scheduled, timeless.as_ref());
        return held(TIMELESS);
    }
    let zone = zone(holding(crate::config::ZONE_KEY).as_deref());
    let falling = |after, until| {
        times
            .as_ref()
            .map_or_else(Fired::default, |times| fired(times, &zone, after, until))
    };
    let started = ctx.stamp();
    let mut kept = kept::read(&directory, service);
    let after = kept
        .considered
        .unwrap_or_else(|| minute_of(started).earlier(MINUTE));
    let due = now || kept.owed || falling(after, started).count > 0;
    if !due || ctx.dry_run {
        return Ok(report(service, ctx.dry_run, due, None));
    }
    kept.owed = false;
    kept::write(&directory, service, &kept)?;
    let ran = ran(ctx, service, started).await;
    let skipped = falling(started, ran.ended);
    kept.owed = kept::read(&directory, service).owed;
    kept.considered = Some(minute_of(ran.ended));
    kept.skipped = kept.skipped.saturating_add(skipped.count);
    kept.last_skipped = skipped.last.or(kept.last_skipped);
    kept.last = Some(ran.clone());
    kept::write(&directory, service, &kept)?;
    let failed = crate::scheduled::failed(service, &ran)
        .map(|problem| crate::scheduled::fault(&problem, ran.exit));
    observed(ctx, &scheduled, failed.as_ref().or(timeless.as_ref()));
    drop(lock);
    Ok(RunReport {
        skipped: skipped.count,
        ran: Some(ran),
        ..report(service, false, true, None)
    })
}

/// Where the records of every service's runs are kept, where anything is configured.
pub(crate) fn records(ctx: &Ctx) -> Option<std::path::PathBuf> {
    super::targets::beside_env(ctx, crate::config::paths::SCHEDULED)
}

/// What `service`'s runs left, or nothing kept where nothing is configured.
pub(crate) fn kept_for(ctx: &Ctx, service: &str) -> Kept {
    records(ctx).map_or_else(Kept::default, |directory| kept::read(&directory, service))
}

/// Every installed run-and-exit service, as its runs left it and this machine's scheduler
/// holds its job, running where `containers` hold a running container of it.
pub(crate) async fn standing(
    ctx: &Ctx,
    installed: &[crate::plugin::Installed],
    containers: &[crate::ports::docker::Container],
) -> Vec<crate::model::ScheduledService> {
    let mut found = kept_standing(ctx, installed, containers);
    for one in &mut found {
        one.fired = ctx
            .seams
            .hosting
            .standing(&crate::scheduled::job(&one.id))
            .await
            .is_ok_and(|held| crate::scheduled::fires(&held));
    }
    found
}

/// Every installed run-and-exit service as its runs left it, read once the scheduler
/// that fires it is asked.
pub(crate) async fn installed_standing(ctx: &Ctx) -> Vec<crate::model::ScheduledService> {
    let installed = super::plugins::read(ctx)
        .map(|register| register.installed().to_vec())
        .unwrap_or_default();
    standing(ctx, &installed, &[]).await
}

/// The doctor's check of each installed run-and-exit service.
pub(crate) fn checked(ctx: &Ctx) -> crate::doctor::scheduled::ScheduledCheck {
    let installed = super::plugins::read(ctx)
        .map(|register| register.installed().to_vec())
        .unwrap_or_default();
    crate::doctor::scheduled::ScheduledCheck::new(
        kept_standing(ctx, &installed, &[]),
        ctx.seams.hosting.clone(),
    )
}

/// The same, with no scheduler asked, so each reads as unfired.
pub(crate) fn kept_standing(
    ctx: &Ctx,
    installed: &[crate::plugin::Installed],
    containers: &[crate::ports::docker::Container],
) -> Vec<crate::model::ScheduledService> {
    installed
        .iter()
        .flat_map(|one| &one.services)
        .filter(|placed| placed.fields.schedule.is_some())
        .map(|placed| {
            let kept = kept_for(ctx, &placed.service);
            crate::model::ScheduledService {
                id: placed.service.clone(),
                name: placed.called().to_owned(),
                running: containers.iter().any(|container| {
                    container.service == placed.service && container.lifecycle == Lifecycle::Running
                }),
                fired: false,
                health: crate::model::RunHealth::of(kept.last.as_ref()),
                last: kept.last,
                skipped: kept.skipped,
            }
        })
        .collect()
}

/// Mark the run-and-exit services a start, a restart or an apply reached owed one run,
/// where it was carried out.
pub(crate) fn started(ctx: &Ctx, action: &Action, report: &crate::model::LifecycleReport) {
    if report.rehearsed || report.status != Some(0) {
        return;
    }
    let reached = match action {
        Action::Start(named) | Action::Restart(named) if !named.is_empty() => Some(named),
        Action::Up | Action::Start(_) | Action::Restart(_) => None,
        Action::Down
        | Action::Stop(_)
        | Action::Remove(_)
        | Action::Pull(_)
        | Action::Once(_)
        | Action::Config => return,
    };
    if let Ok(register) = super::plugins::read(ctx) {
        owe(ctx, register.installed(), reached.map(Vec::as_slice));
    }
}

/// Mark every installed run-and-exit service among `services` owed one run, or every one
/// of them where `services` is `None`.
pub(crate) fn owe(ctx: &Ctx, installed: &[crate::plugin::Installed], services: Option<&[String]>) {
    if ctx.dry_run {
        return;
    }
    let Some(directory) = records(ctx) else {
        return;
    };
    let owed: Vec<String> = crate::scheduled::all(installed)
        .into_iter()
        .map(|one| one.service)
        .filter(|service| services.is_none_or(|named| named.contains(service)))
        .collect();
    kept::owe(&directory, &owed);
}

/// Whether the stack is running: started rather than stopped on purpose, with a
/// container of it running.
async fn up(ctx: &Ctx) -> bool {
    if super::autostart::load(ctx).halted() {
        return false;
    }
    ctx.seams
        .engine
        .list(&ctx.settings.project)
        .await
        .is_ok_and(|containers| {
            containers
                .iter()
                .any(|container| container.lifecycle == Lifecycle::Running)
        })
}

/// Run `service` to completion, from `started`.
async fn ran(ctx: &Ctx, service: &str, started: Instant) -> ScheduledRun {
    let (exit, log) = match said(ctx, &Action::Once(service.to_owned())).await {
        Ok(output) => (
            output.status,
            kept::tail(&format!("{}\n{}", output.stdout, output.stderr)),
        ),
        Err(why) => (None, kept::tail(&why)),
    };
    ScheduledRun {
        started,
        ended: ctx.stamp(),
        exit,
        log,
    }
}

/// What carrying out `action` said, or why it could not be carried out.
async fn said(ctx: &Ctx, action: &Action) -> Result<crate::ports::process::Output, String> {
    let invocation = super::engine::invocation(ctx, &[], action)
        .await
        .map_err(|problem| problem.summary)?;
    super::engine::before(ctx, action, &invocation.addressed).map_err(|problem| problem.summary)?;
    invocation
        .renewal
        .carried_out(ctx)
        .await
        .map_err(|problem| problem.summary)?;
    ctx.seams
        .runner
        .run(&invocation.command)
        .await
        .map_err(|why| why.to_string())
}

/// Put how `scheduled`'s last run went into the conditions, raised by `fault` or cleared.
fn observed(ctx: &Ctx, scheduled: &Scheduled, fault: Option<&Fault>) {
    if ctx.dry_run {
        return;
    }
    let mut conditions = super::conditions::load(ctx);
    conditions.observe(&scheduled.check(), fault, ctx.stamp());
    super::conditions::save(ctx, &conditions);
}

/// A report on `service` with no run in it.
fn report(service: &str, rehearsed: bool, due: bool, held: Option<&str>) -> RunReport {
    RunReport {
        service: service.to_owned(),
        rehearsed,
        due,
        ran: None,
        skipped: 0,
        held: held.map(str::to_owned),
    }
}

/// The problem for a service that is no installed run-and-exit service.
fn not_scheduled(service: &str) -> Problem {
    Problem::new(
        NOT_SCHEDULED,
        format!("{service} is no run-and-exit service"),
        "No installed plugin declares a schedule for it, so it has no times to run at and \
         nothing was run.",
        Remedy::new("List what is installed").with_detail("lemonfiber plugin list"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
