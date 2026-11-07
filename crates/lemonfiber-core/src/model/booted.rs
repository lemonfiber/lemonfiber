//! What became of the last restart of this machine, as `lemonfiber boot` reads it.

use serde::{Deserialize, Serialize};

use super::Forwarded;

/// When this machine last started, what the run after it came to, and how long the
/// stack has failed to come back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct BootReport {
    /// When this machine last started, in seconds since the Unix epoch, where it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub booted_at: Option<u64>,
    /// The verification kept against that start, where one ran after it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<BootVerification>,
    /// The newest verification kept against any start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_verified: Option<BootVerification>,
    /// Why a run after that start started nothing, where one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declined: Option<String>,
    /// Since when the stack has failed to come back, in seconds since the Unix epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failing_since: Option<u64>,
    /// How many starts it has failed to come back after since then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_boots: Option<u32>,
}

/// What one run at boot that reached the stack came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BootVerification {
    /// When the machine started that the run came after, where it said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub against: Option<u64>,
    /// When the run finished.
    pub at: u64,
    /// What it came to.
    pub outcome: BootOutcome,
    /// Each service that did not come back.
    #[serde(default)]
    pub not_back: Vec<String>,
    /// Whether the tunnel came back with the stack.
    pub tunnel: BootTunnel,
    /// The forwarded port before the start and after it, where the stack forwards one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<Forwarded>,
    /// The sentence the run raised, where it raised one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
}

/// What a run at boot came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BootOutcome {
    /// The stack came back, tunnel and forwarded port included.
    Passed,
    /// The stack, or its tunnel, did not come back.
    Failed,
    /// The rest came back, and a part waits for its install from this build.
    WaitingForInstall,
}

/// Whether the tunnel came back with the stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum BootTunnel {
    /// Its checks passed.
    Held,
    /// A check of it failed.
    NotEstablished {
        /// Which, in the stack's words.
        said: String,
    },
    /// Nothing checked it: the stack runs no tunnel, did not come up, or could not be
    /// read.
    NotChecked,
}
