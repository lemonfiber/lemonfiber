//! The closed list of what an alert is, as a client offers a switch for each.
//!
//! An alert's event kind is open and grows with every check. A phone needs a short list it
//! can show as switches, so each alert also carries one of these.

use serde::{Deserialize, Serialize};

use super::class::COMPLETIONS;
use crate::error::Severity;

/// What sort of thing an alert tells the operator.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "NotificationKind")]
pub enum Notification {
    /// A condition at warning or worse, at onset and resolution.
    #[default]
    Problem,
    /// Work the operator asked for finishing.
    Completion,
    /// An update to the stack or lemonfiber being available.
    Update,
    /// A backup that succeeded.
    Backup,
    /// A household member's request newly waiting on the operator.
    Request,
    /// An invitation accepted, declined or lapsed.
    Member,
    /// Something a member asked for has arrived; the one kind a member is told.
    Ready,
}

/// Every notification kind this build raises.
pub const RAISED: [Notification; 1] = [Notification::Problem];

impl Notification {
    /// The notification kind of an event of `kind` at `severity`.
    #[must_use]
    pub fn of(kind: &str, severity: Severity) -> Self {
        if severity < Severity::Warning && COMPLETIONS.contains(&kind) {
            return Self::Completion;
        }
        Self::Problem
    }

    /// What it is called where it is named.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Problem => "problem",
            Self::Completion => "completion",
            Self::Update => "update",
            Self::Backup => "backup",
            Self::Request => "request",
            Self::Member => "member",
            Self::Ready => "ready",
        }
    }
}

#[cfg(test)]
mod tests;
