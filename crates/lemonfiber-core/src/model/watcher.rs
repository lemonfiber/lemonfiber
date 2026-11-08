//! What the outside watcher sees, as `lemonfiber watcher` reads it.

use serde::Serialize;

use crate::ports::service::{Covers, MonitorKind};

/// Which part watches the stack from outside it, whether it answers, and what each of its
/// monitors sees beside what it covers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct WatcherReport {
    /// The plugin filling `status.watch`, where one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watcher: Option<String>,
    /// Whether the watcher answers. False is coverage nobody can see, never all clear.
    pub reachable: bool,
    /// When it was asked, in seconds since the Unix epoch.
    pub read_at: u64,
    /// Why it does not answer, where it does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unreachable_because: Option<String>,
    /// Every monitor it holds, the operator's and lemonfiber's.
    #[serde(default)]
    pub monitors: Vec<WatchedTarget>,
    /// Every critical service and household entry point no active monitor covers.
    #[serde(default)]
    pub uncovered: Vec<Covers>,
}

/// One monitor, and what it covers where lemonfiber made it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct WatchedTarget {
    /// What it covers, by the record of what lemonfiber provisioned; absent for a monitor
    /// lemonfiber did not make.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub covers: Option<Covers>,
    /// Its own name in the watcher.
    pub name: String,
    /// How it checks.
    pub kind: MonitorKind,
    /// What it sees.
    pub state: WatchState,
    /// Since when, in seconds since the Unix epoch.
    pub since: u64,
    /// When it last took a reading, in seconds since the Unix epoch, where it ever has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reading: Option<u64>,
    /// A heartbeat's interval, grace and last check-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat: Option<Heartbeat>,
    /// Whether it is checking, rather than only defined.
    pub active: bool,
    /// Which path an entry point's monitor watches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<WatchPath>,
    /// What it was asked to watch and what it watches instead, where it does not watch
    /// what it should.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unconfigured: Option<Unconfigured>,
}

/// What a monitor sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WatchState {
    /// What it watches answered.
    Up,
    /// What it watches failed as many checks as it alarms after.
    Down,
    /// What it watches failed a check, and fewer than it alarms after.
    Pending,
    /// It is not checking.
    Paused,
    /// What it covers is stopped on purpose.
    ExpectedDown,
    /// It holds no reading newer than its interval and grace.
    NotReporting,
}

/// A heartbeat's interval and grace, and its last check-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Heartbeat {
    /// The seconds between the check-ins it expects.
    pub every_s: u64,
    /// The seconds past its interval it waits before it alarms.
    pub grace_s: u64,
    /// When it last checked in, in seconds since the Unix epoch, where it ever has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_check_in: Option<u64>,
}

/// Which path an entry point's monitor watches.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum WatchPath {
    /// The address the household is handed.
    Household,
    /// The address the stack's own network reaches it at.
    Internal,
}

/// What a monitor was asked to watch, what it watches instead, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Unconfigured {
    /// What it was asked to watch.
    pub wanted: String,
    /// What it watches.
    pub watches: String,
    /// Why, in the stack's words.
    pub why: String,
}
