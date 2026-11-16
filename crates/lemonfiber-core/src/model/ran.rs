//! What one minute's look at a run-and-exit service came to.

use serde::Serialize;

use crate::scheduled::kept::ScheduledRun;

/// What `lemonfiber run` came to for one service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct RunReport {
    /// The service.
    pub service: String,
    /// Whether nothing was run because this was a rehearsal.
    pub rehearsed: bool,
    /// Whether a time fell due or a run was owed, so it ran or would have.
    pub due: bool,
    /// The run, where one went.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ran: Option<ScheduledRun>,
    /// How many times fell due while that run was still going, each skipped.
    pub skipped: u32,
    /// Why nothing was looked at, where something held it back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub held: Option<String>,
}

/// One run-and-exit service, as its runs left it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ScheduledService {
    /// Its id, which is its Compose service name.
    pub id: String,
    /// What it is called in front of an operator.
    pub name: String,
    /// Whether a run of it is going now.
    pub running: bool,
    /// Whether this machine's scheduler holds its job, so its times are fired.
    pub fired: bool,
    /// Its health: how its last run exited.
    pub health: RunHealth,
    /// Its last run, where it has run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<ScheduledRun>,
    /// How many of its times fell due while a run was still going, each skipped.
    pub skipped: u32,
}

impl ScheduledService {
    /// The same service with none of the lines its last run wrote.
    #[must_use]
    pub fn unlogged(self) -> Self {
        Self {
            last: self.last.map(ScheduledRun::unlogged),
            ..self
        }
    }

    /// Whether it reads healthy: its last run exited zero, or it has not run yet.
    #[must_use]
    pub const fn healthy(&self) -> bool {
        !matches!(self.health, RunHealth::Failed)
    }
}

impl RunReport {
    /// The same report with none of the lines its run wrote.
    #[must_use]
    pub fn unlogged(self) -> Self {
        Self {
            ran: self.ran.map(ScheduledRun::unlogged),
            ..self
        }
    }
}

/// How a run-and-exit service's last run exited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RunHealth {
    /// It has not run yet.
    NotRun,
    /// It exited zero.
    Succeeded,
    /// It exited non-zero, or ended before it exited.
    Failed,
}

impl RunHealth {
    /// The health `last` leaves, where it has run.
    #[must_use]
    pub const fn of(last: Option<&ScheduledRun>) -> Self {
        match last {
            None => Self::NotRun,
            Some(ran) if ran.succeeded() => Self::Succeeded,
            Some(_) => Self::Failed,
        }
    }
}
