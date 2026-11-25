//! The backup or restore running on this machine, read from the one reading the copy keeps
//! whichever process runs it and whoever started it.

use serde::{Deserialize, Serialize};

use crate::backup::{Scope, Tree};
use crate::instant::Instant;

mod kept;
pub mod pacing;
pub mod run;

pub use run::Starting;

/// Whether a copy is a backup or a restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "CopyWhat")]
pub enum What {
    /// A backup, writing an archive.
    Backup,
    /// A restore, writing an archive back.
    Restore,
}

/// The part a key is told a copy of an existing setup writes, in place of the host path.
pub const EXISTING_PART: &str = "existing setup configuration";

/// What a running copy copies, as its reading names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
#[schemars(rename = "CopiedScope")]
pub enum Copied {
    /// Every service's configuration, plus lemonfiber's own and the stack.
    WholeStack,
    /// One named service's configuration alone.
    Service {
        /// The service whose configuration it copies.
        name: String,
    },
    /// An existing setup's own configuration.
    Existing {
        /// The Compose project it copies, to the operator alone.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<String>,
        /// The host trees it copies, to the operator alone.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trees: Option<Vec<Tree>>,
    },
}

impl From<Scope> for Copied {
    fn from(scope: Scope) -> Self {
        match scope {
            Scope::WholeStack => Self::WholeStack,
            Scope::Service { name } => Self::Service { name },
            Scope::Existing { project, trees } => Self::Existing {
                project: Some(project),
                trees: Some(trees),
            },
        }
    }
}

/// How a running copy is moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum State {
    /// Writing as fast as it can.
    Working,
    /// Held to the operator's limit.
    Paced {
        /// The limit, in bytes a second.
        limit: u64,
    },
    /// Nothing written for the minutes the operator's setting holds.
    Stalled {
        /// The instant it last wrote.
        wrote: Instant,
    },
}

/// A backup or restore running now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "RunningCopy")]
pub struct Running {
    /// Whether it is a backup or a restore.
    pub what: What,
    /// What it copies.
    pub scope: Copied,
    /// The name an accepting reply gave the work, where one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    /// When it started.
    pub started: Instant,
    /// The bytes written so far.
    pub written: u64,
    /// The bytes it will write.
    pub whole: u64,
    /// The area being written now, named.
    pub part: String,
    /// How it is moving.
    #[serde(flatten)]
    pub state: State,
}

/// The backup or restore running on this machine now, or none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "CopyingReport")]
pub struct Copying {
    /// The copy running now, or none.
    pub running: Option<Running>,
}

impl Running {
    /// The copy as a key is answered with it: no job, which the operator alone redeems,
    /// and for an existing setup neither its project, its trees nor a host path as the part.
    #[must_use]
    pub fn for_a_key(self) -> Self {
        let (scope, part) = match self.scope {
            Copied::Existing { .. } => (
                Copied::Existing {
                    project: None,
                    trees: None,
                },
                EXISTING_PART.to_owned(),
            ),
            scope => (scope, self.part),
        };
        Self {
            job: None,
            scope,
            part,
            ..self
        }
    }
}

impl Copying {
    /// The reading as a key is answered with it.
    #[must_use]
    pub fn for_a_key(self) -> Self {
        Self {
            running: self.running.map(Running::for_a_key),
        }
    }
}

/// What a copy tells as it writes, and what holds a backup to the operator's limit.
pub trait Meter: Send + Sync {
    /// The bytes the whole copy will write, where only the writer can count them.
    fn whole(&self, bytes: u64);

    /// The path inside the archive of the entry being written now.
    fn writing(&self, entry: &str);

    /// Bytes just written; returns once the copy may write more.
    fn wrote(&self, bytes: u64);
}

#[cfg(test)]
mod tests;
