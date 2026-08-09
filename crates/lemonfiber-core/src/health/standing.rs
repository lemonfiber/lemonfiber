//! The one word the stack amounts to, and how the words rank.

use serde::{Deserialize, Serialize};

use crate::error::Severity;

/// What the stack amounts to.
///
/// Ordered from best to worst, so the worst of several is a `max` and there is no
/// second place to encode the ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
    pub const fn wants_attention(self) -> bool {
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
mod tests {
    use super::Standing;
    use crate::error::Severity;

    #[test]
    fn each_severity_has_its_own_standing() {
        for (severity, expected) in [
            (Severity::Advisory, Standing::Advisory),
            (Severity::Warning, Standing::Degraded),
            (Severity::Error, Standing::Broken),
            (Severity::Critical, Standing::Critical),
        ] {
            assert_eq!(Standing::of(severity), expected, "{severity:?}");
        }
    }

    #[test]
    fn the_worst_is_a_max_rather_than_a_ranking_written_out_twice() {
        // Declaration order is the ranking, so a new standing cannot be added in the
        // wrong place and quietly outrank something worse than it.
        let mut ordered = vec![
            Standing::Critical,
            Standing::Healthy,
            Standing::Broken,
            Standing::Degraded,
        ];
        ordered.sort_unstable();
        assert_eq!(
            ordered,
            vec![
                Standing::Healthy,
                Standing::Degraded,
                Standing::Broken,
                Standing::Critical
            ]
        );
    }

    #[test]
    fn every_standing_has_a_word_and_only_the_bad_ones_want_attention() {
        let all = [
            Standing::Healthy,
            Standing::Stopped,
            Standing::Unconfigured,
            Standing::Advisory,
            Standing::Degraded,
            Standing::Broken,
            Standing::Critical,
            Standing::Unknown,
        ];
        for standing in all {
            assert!(!standing.word().is_empty(), "{standing:?}");
        }
        let wanting: Vec<Standing> = all
            .into_iter()
            .filter(|standing| standing.wants_attention())
            .collect();
        assert_eq!(
            wanting,
            vec![Standing::Degraded, Standing::Broken, Standing::Critical],
            "an advisory is worth knowing and not worth acting on"
        );
    }
}
