//! The one word the stack amounts to, and how the words rank.

use serde::{Deserialize, Serialize};

use crate::error::Severity;

/// What the stack amounts to.
///
/// Ordered from best to worst, so the worst of several is a `max` and there is no
/// second place to encode the ranking.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "HealthStanding")]
pub enum Standing {
    /// Nothing is wrong.
    Healthy,
    /// Nothing is running, and that was on purpose.
    Stopped,
    /// Nothing is set up.
    Unconfigured,
    /// Worth knowing, nothing to do.
    Advisory,
    /// Working, with something wrong.
    Degraded,
    /// Something is broken.
    Broken,
    /// Something is wrong outside this machine, or data is at risk.
    Critical,
    /// It could not be established. Never reported as healthy.
    Unknown,
}

impl Standing {
    /// The standing a severity amounts to on its own.
    pub(super) const fn of(severity: Severity) -> Self {
        match severity {
            Severity::Advisory => Self::Advisory,
            Severity::Warning => Self::Degraded,
            Severity::Error => Self::Broken,
            Severity::Critical => Self::Critical,
        }
    }

    /// Whether this is a state an operator has to do something about.
    #[must_use]
    pub(crate) const fn wants_attention(self) -> bool {
        matches!(self, Self::Degraded | Self::Broken | Self::Critical)
    }

    /// The word an operator reads.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Stopped => "stopped",
            Self::Unconfigured => "not set up",
            Self::Advisory => "worth a look",
            Self::Degraded => "degraded",
            Self::Broken => "broken",
            Self::Critical => "critical",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests;
