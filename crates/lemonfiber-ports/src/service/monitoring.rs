//! The outside watcher: a monitor that watches the stack from outside it, holding the
//! monitors lemonfiber provisions under its own tags beside those the operator made.

use async_trait::async_trait;

use super::Failure;

/// What every tag lemonfiber gives a monitor begins with.
pub const TAG_PREFIX: &str = "lemonfiber:";

/// How a monitor checks what it watches.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum MonitorKind {
    /// An HTTP request answered with a success.
    Http,
    /// An HTTP request whose answer holds a word.
    Keyword,
    /// A TCP port that takes a connection.
    Port,
    /// A host that answers a ping.
    Ping,
    /// A name that resolves.
    Dns,
    /// A container that runs.
    Docker,
    /// A check-in expected each interval.
    Heartbeat,
}

/// What a monitor last found.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum MonitorState {
    /// What it watches answered.
    Up,
    /// What it watches failed as many checks as it alarms after.
    Down,
    /// What it watches failed a check, and fewer than it alarms after.
    Pending,
    /// It is not checking.
    Paused,
}

/// A heartbeat's interval and grace, and its last check-in.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Beat {
    /// The seconds between the check-ins it expects.
    pub every_s: u64,
    /// The seconds past its interval it waits before it alarms.
    pub grace_s: u64,
    /// When it last checked in, in seconds since the Unix epoch, or nothing where it never
    /// has.
    pub last_check_in: Option<u64>,
}

/// One monitor the watcher holds.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Monitor {
    /// The watcher's id for it.
    pub id: String,
    /// The tag lemonfiber gave it, where lemonfiber made it.
    pub tag: Option<String>,
    /// Its own name in the watcher.
    pub name: String,
    /// How it checks.
    pub kind: MonitorKind,
    /// What it is pointed at.
    pub target: String,
    /// Whether it is checking, rather than only defined.
    pub active: bool,
    /// A heartbeat's interval, grace and last check-in.
    pub heartbeat: Option<Beat>,
    /// What it last found.
    pub state: MonitorState,
    /// Since when, in seconds since the Unix epoch.
    pub since: u64,
    /// When it last took a reading, in seconds since the Unix epoch, or nothing where it
    /// never has.
    pub last_reading: Option<u64>,
}

/// Whether the watcher answers, and why not where it does not.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Reach {
    /// Whether it answers.
    pub reachable: bool,
    /// Why not, in words, where it does not.
    pub why: Option<String>,
}

/// The one monitor to hold a tag.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Provision {
    /// The tag, beginning with [`TAG_PREFIX`].
    pub tag: String,
    /// How it checks.
    pub kind: MonitorKind,
    /// What it is pointed at: an address, a host and its port, a host, a name, a
    /// container's name, or for a heartbeat the id of the job that checks in.
    pub target: String,
    /// The seconds between its checks, or between a heartbeat's check-ins.
    pub every_s: u64,
    /// The seconds past its interval a heartbeat waits before it alarms.
    pub grace_s: Option<u64>,
    /// How many checks in a row fail before it alarms.
    pub retries: u32,
}

/// What the watcher watches in place of what it was asked to, and why.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Instead {
    /// What it watches.
    pub watches: String,
    /// Why, in words.
    pub why: String,
}

/// The monitor a provision made or updated.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Provisioned {
    /// The watcher's id for it.
    pub id: String,
    /// What it watches instead, where it could not watch what it was asked to.
    pub instead: Option<Instead>,
}

/// What a monitor lemonfiber provisions covers, which its tag names.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Covers {
    /// A service of the stack, by its id.
    Service(String),
    /// A household-facing entry point, by the id of the service it is.
    EntryPoint(String),
    /// A job the stack runs on a schedule, by its id.
    Job(String),
}

impl Covers {
    /// The word its tag names it by.
    const fn word(&self) -> &'static str {
        match self {
            Self::Service(_) => "service",
            Self::EntryPoint(_) => "entry",
            Self::Job(_) => "job",
        }
    }

    /// The id of what it covers.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Service(id) | Self::EntryPoint(id) | Self::Job(id) => id,
        }
    }

    /// The tag lemonfiber gives its monitor.
    #[must_use]
    pub fn tag(&self) -> String {
        format!("{TAG_PREFIX}{}:{}", self.word(), self.id())
    }

    /// What `tag` names, where it is one of lemonfiber's.
    #[must_use]
    pub fn read(tag: &str) -> Option<Self> {
        let (word, id) = tag.strip_prefix(TAG_PREFIX)?.split_once(':')?;
        if id.is_empty() {
            return None;
        }
        let id = id.to_owned();
        match word {
            "service" => Some(Self::Service(id)),
            "entry" => Some(Self::EntryPoint(id)),
            "job" => Some(Self::Job(id)),
            _ => None,
        }
    }
}

/// The outside watcher, asked what it sees and told what to watch.
#[async_trait]
pub trait Monitoring: Send + Sync {
    /// Whether it answers, and why not where it does not.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where its adapter is unreachable or answers unreadably.
    async fn reachable(&self) -> Result<Reach, Failure>;

    /// Every monitor it holds, the operator's and lemonfiber's.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn monitors(&self) -> Result<Vec<Monitor>, Failure>;

    /// Make or update the one monitor holding `provision`'s tag, leaving one monitor for
    /// a tag provisioned twice.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unsupported`] where its monitors cannot be configured, and
    /// [`Failure`] where it is unreachable or refuses.
    async fn provision(&self, provision: &Provision) -> Result<Provisioned, Failure>;

    /// Record a check-in of the heartbeat holding `tag`, as the job it watches makes one
    /// each interval.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unsupported`] where its monitors cannot be configured, and
    /// [`Failure`] where no heartbeat holds `tag`, or it is unreachable or refuses.
    async fn check_in(&self, tag: &str) -> Result<(), Failure>;

    /// Remove the monitor holding `tag`, and nothing where none does.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unsupported`] where its monitors cannot be configured, and
    /// [`Failure`] where it is unreachable or refuses.
    async fn retire(&self, tag: &str) -> Result<(), Failure>;
}

#[cfg(test)]
mod tests;
