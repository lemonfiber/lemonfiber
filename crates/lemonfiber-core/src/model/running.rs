//! What starting, stopping and resetting the stack answer with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// What a lifecycle command did, or would have done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleReport {
    /// The Compose subcommand that was run.
    pub action: String,
    /// The profiles that were activated.
    pub profiles: Vec<String>,
    /// Profiles the forms asked for that the configuration does not support.
    ///
    /// Reported rather than dropped quietly: an operator seeing fewer services
    /// than they expected needs to be told which, and why, before they go
    /// looking for a fault that is not there.
    pub dropped: Vec<String>,
    /// The exact command, so what happened is never a matter of trust.
    pub command: Vec<String>,
    /// Whether this was a rehearsal.
    pub rehearsed: bool,
    /// The exit status, absent for a rehearsal or a signalled process.
    pub status: Option<i32>,
    /// What each service ended up doing, where the action waited to find out.
    ///
    /// Empty for actions that do not wait. Stopping is finished when Compose
    /// says it is, and surveying afterwards would only report the absence it
    /// was asked to produce.
    pub services: Vec<crate::docker::Service>,
    /// What those services amount to, as one word.
    pub condition: Option<crate::docker::Condition>,
    /// Stack files the operator has edited, left as they set them rather than
    /// overwritten with lemonfiber's own. Empty in the ordinary case; a named entry
    /// warns that an upgrade would change a file they changed, and shows the diff.
    pub stack_edits: Vec<StackEdit>,
    /// What starting the stack did about the VPN's forwarded port, where it did
    /// anything. Absent in the ordinary case — the client was already on it, or
    /// there is no tunnel to forward through — and a sentence where the client was
    /// moved, or could not be.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forwarding: Option<String>,
}

/// A stack file the operator edited, preserved rather than overwritten, with the
/// change an upgrade would make shown against it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StackEdit {
    /// The file's path within the stack directory.
    pub path: String,
    /// The lines that differ between the operator's file and what lemonfiber would
    /// write — theirs marked `-`, lemonfiber's `+`, the matching head and tail left
    /// out. Empty where the two differ only in ways `lines` does not see.
    pub diff: String,
}

/// What a full reset did, or — until it is confirmed — would do: the operator edits it
/// reverts back to lemonfiber's own state, and whether it was carried out or only shown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResetReport {
    /// The operator's edits that were reverted — or, unconfirmed, that a reset would
    /// revert — each with the diff of what is lost against what lemonfiber restores.
    pub reverted: Vec<StackEdit>,
    /// The service connections whose drifted value was reverted to lemonfiber's — or,
    /// unconfirmed, would be — each named as it reads in a seed report.
    pub reverted_connections: Vec<String>,
    /// Whether the reset was carried out, or only previewed pending confirmation.
    pub confirmed: bool,
}
