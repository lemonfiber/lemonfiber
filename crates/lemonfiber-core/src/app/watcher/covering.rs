//! What the outside watcher should watch: every critical service, every household entry
//! point over the address the household is handed, and every job this machine hosts that
//! acts on a beat.

use lemonfiber_manifest::{Criticality, HealthKind, Service};

use crate::app::command::Hostable;
use crate::app::Ctx;
use crate::door::{candidates, Candidate, Place};
use crate::model::{Unconfigured, WatchPath};
use crate::plugin::Installed;
use crate::ports::hosting::Standing;
use crate::ports::service::{Covers, MonitorKind, Provision};
use crate::wiring::Fillers;

/// The seconds between the checks of a service or an entry point.
const EVERY_S: u64 = 60;

/// How many checks in a row fail before a service or an entry point's monitor alarms, so
/// one slow answer on a busy line raises nothing.
const RETRIES: u32 = 3;

/// How many check-ins in a row a job misses, past its grace, before its heartbeat alarms:
/// none, since its grace is already the allowance.
const MISSED: u32 = 0;

/// What is said a monitor was asked to watch where it watches an entry point over the
/// stack's own network instead.
const HOUSEHOLD_PATH: &str = "the address the household is handed";

/// A command this machine hosts that acts on a beat, and checks in at each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Job {
    /// The hosted command.
    pub(crate) hosted: Hostable,
    /// The id its tag names it by.
    pub(crate) id: &'static str,
    /// The seconds between its check-ins.
    pub(crate) every_s: u64,
    /// The seconds past them its heartbeat waits before it alarms.
    pub(crate) grace_s: u64,
}

impl Job {
    /// What its heartbeat covers.
    pub(crate) fn covers(self) -> Covers {
        Covers::Job(self.id.to_owned())
    }
}

/// The guard on the data location, which checks in each minute it holds.
pub(crate) const GUARD: Job = Job {
    hosted: Hostable::Watch,
    id: "guard",
    every_s: 60,
    grace_s: 60,
};

/// The clock that closes requests nobody ruled on, which checks in at each hourly wake.
pub(crate) const EXPIRING: Job = Job {
    hosted: Hostable::Expiring,
    id: "expiring",
    every_s: 3_600,
    grace_s: 3_600,
};

/// Every job that checks in.
pub(crate) const JOBS: [Job; 2] = [GUARD, EXPIRING];

/// One monitor the watcher should hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wanted {
    /// What it covers.
    pub(crate) covers: Covers,
    /// What it is provisioned as.
    pub(crate) provision: Provision,
    /// Which path an entry point's monitor watches.
    pub(crate) path: Option<WatchPath>,
    /// Where it cannot watch what it should, as the core provisions it.
    pub(crate) short: Option<Unconfigured>,
}

impl Wanted {
    /// A monitor of `kind` at `target`, checked each minute.
    fn checked(covers: Covers, kind: MonitorKind, target: String) -> Self {
        Self {
            provision: Provision {
                tag: covers.tag(),
                kind,
                target,
                every_s: EVERY_S,
                grace_s: None,
                retries: RETRIES,
            },
            covers,
            path: None,
            short: None,
        }
    }
}

/// Every monitor the watcher should hold for the stack's `services`, the `installed`
/// plugins' and the jobs this machine hosts, in that order.
pub(crate) async fn wanted(
    ctx: &Ctx,
    services: &[Service],
    installed: &[Installed],
    fillers: &Fillers,
) -> Vec<Wanted> {
    let named = ctx.site.name().await;
    let (host, domain) = crate::settings::household_place(ctx);
    let place = Place {
        named: named.as_deref(),
        recorded: host.as_deref(),
        domain: domain.as_deref(),
        environment: ctx.environment,
    };
    let mut wanted: Vec<Wanted> = services
        .iter()
        .filter(|service| critical(service))
        .map(|service| watched(&ctx.settings.project, service, fillers))
        .collect();
    wanted.extend(
        candidates(services, installed, fillers)
            .iter()
            .filter(|candidate| {
                candidate
                    .facing
                    .is_some_and(crate::door::Facing::handed_over)
            })
            .filter_map(|candidate| entered(candidate, &place, fillers)),
    );
    for job in JOBS {
        if hosted(ctx, job).await {
            wanted.push(beating(job));
        }
    }
    wanted
}

/// Whether `service` is one the watcher covers: its failure has consequences outside the
/// machine, or its form does not work without it.
fn critical(service: &Service) -> bool {
    matches!(
        service.criticality,
        Criticality::Critical | Criticality::Core
    )
}

/// The monitor of a critical service: its health path where it answers one, its port
/// where it takes connections, and its container otherwise.
fn watched(project: &str, service: &Service, fillers: &Fillers) -> Wanted {
    let covers = Covers::Service(service.id.clone());
    let address = fillers
        .service(&service.id)
        .and_then(|filler| filler.address.as_ref());
    let health = service.health.as_ref();
    match (address, health.map(|health| health.kind)) {
        (Some(address), Some(HealthKind::Http)) => {
            let path = health
                .and_then(|health| health.path.as_deref())
                .unwrap_or("/");
            Wanted::checked(
                covers,
                MonitorKind::Http,
                format!("{}{path}", address.url()),
            )
        }
        (Some(address), Some(HealthKind::Tcp) | None) => Wanted::checked(
            covers,
            MonitorKind::Port,
            format!("{}:{}", address.host, address.port),
        ),
        (Some(_), Some(HealthKind::Container)) | (None, _) => Wanted::checked(
            covers,
            MonitorKind::Docker,
            format!("{project}-{}-1", service.id),
        ),
    }
}

/// The monitor of a household entry point: at the address the household is handed, or
/// where nothing here can work out that address, at the stack's own address for it and
/// said to be so; nothing where neither is known.
fn entered(candidate: &Candidate<'_>, place: &Place<'_>, fillers: &Fillers) -> Option<Wanted> {
    let covers = Covers::EntryPoint(candidate.id.to_owned());
    if let Some(address) = crate::door::run::reached(candidate, place) {
        let mut wanted = Wanted::checked(covers, MonitorKind::Http, address.url);
        wanted.path = Some(WatchPath::Household);
        return Some(wanted);
    }
    let internal = fillers.service(candidate.id)?.address.as_ref()?.url();
    let mut wanted = Wanted::checked(covers, MonitorKind::Http, internal.clone());
    wanted.path = Some(WatchPath::Internal);
    wanted.short = Some(Unconfigured {
        wanted: HOUSEHOLD_PATH.to_owned(),
        watches: internal,
        why: crate::door::run::unaddressed(candidate).trim().to_owned(),
    });
    Some(wanted)
}

/// Whether the service manager hosts `job`, so it is expected to run.
async fn hosted(ctx: &Ctx, job: Job) -> bool {
    ctx.seams
        .hosting
        .standing(job.hosted.name())
        .await
        .is_ok_and(|held| held.standing != Standing::Absent)
}

/// The heartbeat of `job`.
fn beating(job: Job) -> Wanted {
    let covers = job.covers();
    Wanted {
        provision: Provision {
            tag: covers.tag(),
            kind: MonitorKind::Heartbeat,
            target: job.id.to_owned(),
            every_s: job.every_s,
            grace_s: Some(job.grace_s),
            retries: MISSED,
        },
        covers,
        path: None,
        short: None,
    }
}

#[cfg(test)]
mod tests;
