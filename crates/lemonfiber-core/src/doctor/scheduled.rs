//! How each run-and-exit service's last run ended.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::codes::host::SCHEDULED_RUN_FAILED;
use crate::error::{Problem, Remedy};
use crate::model::ScheduledService;
use crate::ports::hosting::Host;

/// The heading an operator reads each of these under.
const TITLE: &str = "A service run on a schedule";

/// One finding per run-and-exit service: a pass where its last run exited zero or it has
/// not run yet, a warning with its exit and last lines where its last run did not, and a
/// warning where nothing on this machine fires its times.
pub struct ScheduledCheck {
    /// Each run-and-exit service, as its runs left it.
    services: Vec<ScheduledService>,
    /// The machine's scheduler, asked whether it holds each one's job.
    hosting: Arc<dyn Host>,
}

impl ScheduledCheck {
    /// A check reporting on `services`, asking `hosting` whether it fires each.
    #[must_use]
    pub fn new(services: Vec<ScheduledService>, hosting: Arc<dyn Host>) -> Self {
        Self { services, hosting }
    }
}

#[async_trait]
impl Check for ScheduledCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        findings(&self.services, self.hosting.as_ref()).await
    }
}

/// One finding for each of `services`, asking `hosting` whether it fires each.
async fn findings(services: &[ScheduledService], hosting: &dyn Host) -> Vec<Finding> {
    let mut found = Vec::with_capacity(services.len());
    for one in services {
        let fired = hosting
            .standing(&crate::scheduled::job(&one.id))
            .await
            .is_ok_and(|held| crate::scheduled::fires(&held));
        found.push(finding(one, fired));
    }
    found
}

/// What `one`'s last run comes to, and whether `fired`.
fn finding(one: &ScheduledService, fired: bool) -> Finding {
    let failed = one
        .last
        .as_ref()
        .and_then(|last| crate::scheduled::failed(&one.id, last))
        .or_else(|| (!fired).then(|| unfired(&one.id)));
    let verdict = failed.map_or_else(
        || Verdict::Pass {
            note: Some(one.last.as_ref().map_or_else(
                || format!("{} has not run yet", one.id),
                |last| format!("{} last ran at {}", one.id, last.ended.said()),
            )),
        },
        Verdict::Warn,
    );
    Finding::in_category(
        Category::Services,
        &crate::scheduled::checked(&one.id),
        TITLE,
        verdict,
    )
}

/// What a run-and-exit service nothing on this machine fires comes to.
fn unfired(service: &str) -> Problem {
    Problem::new(
        SCHEDULED_RUN_FAILED,
        format!("Nothing on this machine runs {service} at its times"),
        format!(
            "This machine's scheduler holds no job for {service}, so what it runs to do is not \
             being done."
        ),
        Remedy::new("Read what this machine keeps running, which says why")
            .with_detail("lemonfiber hosting"),
    )
}

#[cfg(test)]
mod tests;
