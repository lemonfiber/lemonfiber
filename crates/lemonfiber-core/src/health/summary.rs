//! The one-line summary, computed once so every surface says the same thing.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{Reach, Standing};
use crate::condition::Condition;
use crate::error::Severity;

/// How long a fault must have been gone before the summary calls it gone, in
/// seconds.
///
/// The debounce runs on the *clearing* side, not the appearing side. A fault that
/// has only just appeared still counts immediately — holding it back would let a
/// stack with an unverified tunnel read "healthy" for the first half-minute, and
/// silence claimed as health is the one thing this must never do. What flaps is a
/// service bouncing every few seconds, and what makes it flap in the *summary* is
/// declaring it fixed in the gaps. So a fault that has come back before is still
/// counted for a while after it clears: one continuous problem rather than a word
/// that changes twice a minute.
pub(crate) const STEADY: u64 = 30;

/// The one-line summary, and what it expands to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(rename = "HealthSummary")]
pub struct Summary {
    /// The one word.
    pub standing: Standing,
    /// How many things are wrong — root causes, counted once each, so a disk that
    /// filled and the nine imports that then failed is one thing and not ten.
    pub wanting_attention: usize,
    /// The worst thing, named, so the line says something rather than only
    /// grading. Absent where nothing is wrong.
    pub worst: Option<String>,
    /// Everything that is wrong, worst first, so the line expands to the affected
    /// items and their remedies rather than to a number nobody can act on.
    pub affected: Vec<Affected>,
}

/// One thing that is wrong, as the expanded summary lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Affected {
    /// The check that raised it.
    pub check: String,
    /// How bad it is.
    pub severity: Severity,
    /// What is wrong, in one line.
    pub summary: String,
    /// What it costs the operator. The line expands to items an operator can act
    /// on, and an item that states only the event leaves the judgement it was
    /// supposed to save them.
    pub meaning: String,
    /// What to do about it, most likely first.
    pub remedies: Vec<String>,
    /// What is also wrong because of this, counted with it rather than again.
    pub downstream: Vec<String>,
}

impl Summary {
    /// Summarise, from what is wrong and how far the stack got.
    ///
    /// How far the stack got settles only the cases where "wrong" has no meaning
    /// yet: nothing set up, nothing running on purpose, still coming up, or nobody
    /// could look. Everywhere else the conditions decide, because a container being
    /// up is not evidence that what it is doing is right.
    ///
    /// Takes every condition the store knows, raised or not, and decides for itself
    /// which still count — a fault that has been flapping is not called fixed the
    /// moment it blinks off (see [`STEADY`]). A fault downstream of another that is
    /// also counted is folded into its root, so the number is of problems rather
    /// than of symptoms.
    #[must_use]
    pub fn of(reach: Reach, known: &[&Condition], now: &str) -> Self {
        let steady: Vec<&&Condition> = known
            .iter()
            .filter(|condition| Self::counts(condition, now))
            .collect();
        // Only a root that is itself being reported can absorb anything: folding
        // into one that was debounced away would hide both.
        let roots: BTreeSet<&str> = steady
            .iter()
            .map(|condition| condition.check.as_str())
            .collect();

        let mut affected: Vec<Affected> = steady
            .iter()
            .filter(|condition| !Self::is_folded(condition, &roots, &steady))
            .map(|condition| Affected {
                check: condition.check.clone(),
                severity: condition.severity,
                summary: condition.summary.clone(),
                meaning: condition.meaning.clone(),
                remedies: condition.remedies.clone(),
                downstream: Self::downstream_of(&condition.check, &steady),
            })
            .collect();
        // Worst first, and stably, so two things equally wrong keep the order the
        // checks raised them in rather than an arbitrary one that moves each refresh.
        affected.sort_by_key(|item| Reverse(item.severity));

        let worst = affected.first().map(|first| first.severity);
        let standing = Self::standing(reach, worst);
        Self {
            standing,
            wanting_attention: affected.len(),
            worst: affected.first().map(|first| first.summary.clone()),
            affected,
        }
    }

    /// Whether a condition still counts towards the summary.
    ///
    /// Raised, obviously. And also one that cleared moments ago having come back
    /// before: a service bouncing every few seconds is one continuous problem, and
    /// declaring it fixed in the gaps is exactly the flapping to be avoided. A fault
    /// that cleared and has stayed clear is done.
    ///
    /// A stamp that cannot be read counts as settled, so a clock problem cannot pin
    /// a resolved fault to the summary forever.
    fn counts(condition: &Condition, now: &str) -> bool {
        condition.is_raised()
            || (condition.recurrences > 0
                && condition
                    .settled_for(now)
                    .is_some_and(|settled| settled < STEADY))
    }

    /// Whether this is a symptom of something else that is also being reported.
    ///
    /// Only into a root at least as bad as itself. Folding a critical finding under
    /// an error would let the cascade rule bury the worst thing on the machine,
    /// which is the one outcome the whole summary exists to prevent.
    fn is_folded(condition: &Condition, roots: &BTreeSet<&str>, steady: &[&&Condition]) -> bool {
        let Some(cause) = condition.caused_by.as_deref() else {
            return false;
        };
        roots.contains(cause)
            && steady
                .iter()
                .any(|root| root.check == cause && root.severity >= condition.severity)
    }

    /// What is wrong because of this one, in the words the operator would read.
    fn downstream_of(check: &str, steady: &[&&Condition]) -> Vec<String> {
        steady
            .iter()
            .filter(|other| other.caused_by.as_deref() == Some(check))
            .filter(|other| other.severity <= Self::severity_at(check, steady))
            .map(|other| other.summary.clone())
            .collect()
    }

    /// The severity of the condition filed under a check, where it is being
    /// reported. Advisory where it is not, which absorbs nothing.
    fn severity_at(check: &str, steady: &[&&Condition]) -> Severity {
        steady
            .iter()
            .find(|condition| condition.check == check)
            .map_or(Severity::Advisory, |condition| condition.severity)
    }

    /// The one word, from how far the stack got and the worst thing wrong with it.
    ///
    /// A critical finding outranks every reason to stay quiet: a leak while the
    /// stack is "starting" is still a leak.
    const fn standing(reach: Reach, worst: Option<Severity>) -> Standing {
        match (reach, worst) {
            (_, Some(Severity::Critical)) => Standing::Critical,
            (Reach::Unconfigured, _) => Standing::Unconfigured,
            (Reach::Unreachable | Reach::Starting, _) => Standing::Unknown,
            (Reach::Stopped, _) => Standing::Stopped,
            (Reach::Running, Some(severity)) => Standing::of(severity),
            (Reach::Running, None) => Standing::Healthy,
        }
    }

    /// The line itself.
    ///
    /// A healthy stack gets as clear a sentence as a broken one — "healthy" said
    /// plainly, not an absence of complaint the operator has to interpret.
    #[must_use]
    pub fn said(&self) -> String {
        let Some(worst) = &self.worst else {
            return self.standing.word().to_owned();
        };
        let others = self.wanting_attention.saturating_sub(1);
        if others == 0 {
            return format!("{} — {worst}", self.standing.word());
        }
        format!(
            "{} — {worst}, and {others} other{}",
            self.standing.word(),
            crate::plural::s(others)
        )
    }
}

#[cfg(test)]
mod tests;
