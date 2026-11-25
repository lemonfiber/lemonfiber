//! Run-and-exit services: which are installed, when each falls due, and what its runs
//! left behind.

pub(crate) mod fires;
pub mod kept;

use lemonfiber_manifest::schedule::{Cron, Schedule};

use crate::condition::Fault;
use crate::error::codes::host::SCHEDULED_RUN_FAILED;
use crate::error::{Problem, Remedy};
use crate::plugin::Installed;

use kept::ScheduledRun;

/// The check a run-and-exit service's last run is filed under, before the service's id.
pub const CHECK: &str = "services.scheduled.";

/// One installed run-and-exit service and the schedule its plugin declared for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scheduled {
    /// Its id, which is its Compose service name.
    pub(crate) service: String,
    /// The setting naming its times, and the times where the setting holds none.
    pub(crate) schedule: Schedule,
}

impl Scheduled {
    /// The times it runs at: what the setting holds, or the declared default where it
    /// holds nothing, or nothing where that names no time.
    pub(crate) fn times(&self, held: &dyn Fn(&str) -> Option<String>) -> Option<Cron> {
        let value = held(&self.schedule.setting).filter(|value| !value.trim().is_empty());
        Cron::parse(value.as_deref().unwrap_or(&self.schedule.default))
    }

    /// The check its last run is filed under.
    pub(crate) fn check(&self) -> String {
        checked(&self.service)
    }
}

/// The name this machine's scheduler holds `service`'s job under.
pub(crate) fn job(service: &str) -> String {
    format!("run-{service}")
}

/// What `service`'s job does, in one sentence.
pub(crate) fn guarantees(service: &str) -> String {
    format!("runs {service} at the times its schedule names")
}

/// Whether `held` is a job this machine's scheduler fires.
pub(crate) fn fires(held: &crate::ports::hosting::Held) -> bool {
    held.standing == crate::ports::hosting::Standing::Running && !held.orphaned()
}

/// The check the last run of `service` is filed under.
pub(crate) fn checked(service: &str) -> String {
    format!("{CHECK}{service}")
}

/// Every installed run-and-exit service, in the order the register lists them.
pub(crate) fn all(installed: &[Installed]) -> Vec<Scheduled> {
    installed
        .iter()
        .flat_map(|one| &one.services)
        .filter_map(|placed| {
            placed.fields.schedule.as_ref().map(|schedule| Scheduled {
                service: placed.service.clone(),
                schedule: schedule.clone(),
            })
        })
        .collect()
}

/// Every run-and-exit service of `was` that `would` no longer runs on a schedule.
pub(crate) fn dropped(was: &Installed, would: &Installed) -> Vec<String> {
    let staying = all(std::slice::from_ref(would));
    all(std::slice::from_ref(was))
        .into_iter()
        .map(|one| one.service)
        .filter(|service| !staying.iter().any(|one| one.service == *service))
        .collect()
}

/// The installed run-and-exit service `service` names, where it names one.
pub(crate) fn named(installed: &[Installed], service: &str) -> Option<Scheduled> {
    all(installed)
        .into_iter()
        .find(|one| one.service == service)
}

/// The service whose schedule reads `setting`, where one does.
pub(crate) fn reading<'a>(scheduled: &'a [Scheduled], setting: &str) -> Option<&'a Scheduled> {
    scheduled.iter().find(|one| one.schedule.setting == setting)
}

/// What `service`'s last run comes to where it did not exit zero, or nothing where it
/// did.
pub(crate) fn failed(service: &str, ran: &ScheduledRun) -> Option<Problem> {
    if ran.succeeded() {
        return None;
    }
    let summary = ran.exit.map_or_else(
        || format!("The last scheduled run of {service} ended before it exited"),
        |exit| format!("The last scheduled run of {service} exited with {exit}"),
    );
    let problem = Problem::new(
        SCHEDULED_RUN_FAILED,
        summary,
        format!(
            "What {service} runs to do was not done this time. It runs again at its next time."
        ),
        Remedy::new("Deal with what its last lines say, then run it again")
            .with_detail(format!("lemonfiber restart {service}")),
    );
    Some(if ran.log.is_empty() {
        problem
    } else {
        problem.with_detail(ran.log.join("\n"))
    })
}

/// What a schedule setting naming no time comes to.
pub(crate) fn timeless(scheduled: &Scheduled) -> Problem {
    let service = &scheduled.service;
    let setting = &scheduled.schedule.setting;
    Problem::new(
        SCHEDULED_RUN_FAILED,
        format!("{service} is not running at any time: {setting} names none"),
        format!("What {service} runs to do is not being done."),
        Remedy::new(
            "Set it to a five-field cron expression or one of @hourly, @daily, @weekly and \
             @monthly",
        )
        .with_detail(format!("lemonfiber settings set {setting}=@daily")),
    )
}

/// The condition `problem` raises, carrying `exit`.
pub(crate) fn fault(problem: &Problem, exit: Option<i32>) -> Fault {
    let mut fault = Fault::new(
        problem.code.as_str(),
        problem.severity,
        &problem.summary,
        &problem.meaning,
        problem
            .remedies
            .first()
            .map_or("", |remedy| remedy.action.as_str()),
    );
    fault.exit = exit;
    fault
}

impl crate::app::Outcome {
    /// The outcome as a key that is no member's is answered with it: every run it carries,
    /// and every doctor finding on one, without the lines the run wrote, which only the
    /// operator is handed; a running copy without what only the operator's reads name.
    #[must_use]
    pub fn for_a_key(self) -> Self {
        match self {
            Self::Status(report) => Self::Status(crate::model::StatusReport {
                scheduled: unlogged(report.scheduled),
                ..report
            }),
            Self::Ran(report) => Self::Ran(report.unlogged()),
            Self::Copying(copying) => Self::Copying(copying.for_a_key()),
            Self::Doctor(report) => Self::Doctor(crate::model::DoctorReport {
                findings: report.findings.into_iter().map(unlogged_finding).collect(),
                ..report
            }),
            other => other,
        }
    }
}

/// `finding` without the lines a run-and-exit service's last run wrote, where it is one
/// of theirs.
fn unlogged_finding(mut finding: crate::doctor::Finding) -> crate::doctor::Finding {
    if finding.check.starts_with(CHECK) {
        if let crate::doctor::Verdict::Warn(problem) | crate::doctor::Verdict::Fail(problem) =
            &mut finding.verdict
        {
            problem.detail = None;
        }
    }
    finding
}

/// Each of `services` without the lines its last run wrote.
#[must_use]
pub fn unlogged(
    services: Vec<crate::model::ScheduledService>,
) -> Vec<crate::model::ScheduledService> {
    services
        .into_iter()
        .map(crate::model::ScheduledService::unlogged)
        .collect()
}

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;
