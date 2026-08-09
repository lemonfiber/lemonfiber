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
pub const STEADY: u64 = 30;

/// The one-line summary, and what it expands to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Affected {
    /// The check that raised it.
    pub check: String,
    /// How bad it is.
    pub severity: Severity,
    /// What is wrong, in one line.
    pub summary: String,
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
            if others == 1 { "" } else { "s" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Reach, Standing, Summary, STEADY};
    use crate::condition::{Condition, Fault};
    use crate::error::Severity;

    /// When the faults in these tests were raised, in the seconds-since-the-epoch
    /// the clock port hands out.
    const RAISED: &str = "1000";

    /// Long enough after `RAISED` that everything has settled — so a test that is
    /// not about the debounce is not silently subject to it.
    const SETTLED: &str = "9000";

    /// One thing wrong, at a severity.
    fn wrong(check: &str, severity: Severity, summary: &str) -> Condition {
        Condition::raised(check, &Fault::new(severity, summary, "look at it"), RAISED)
    }

    /// One thing wrong, downstream of another.
    fn caused_by(check: &str, severity: Severity, summary: &str, cause: &str) -> Condition {
        Condition::raised(
            check,
            &Fault::new(severity, summary, "look at it").caused_by(cause),
            RAISED,
        )
    }

    #[test]
    fn a_running_stack_with_nothing_wrong_is_healthy_and_says_so() {
        // As clear a sentence as a broken one, rather than an absence of complaint
        // the operator has to interpret.
        let summary = Summary::of(Reach::Running, &[], SETTLED);
        assert_eq!(summary.standing, Standing::Healthy);
        assert_eq!(summary.said(), "healthy");
        assert!(!summary.standing.wants_attention());
        assert!(summary.affected.is_empty());
    }

    #[test]
    fn everything_running_with_a_critical_finding_is_not_healthy() {
        // The case the whole feature exists for: sixteen containers up and answering
        // while traffic leaves outside the tunnel.
        let leak = wrong(
            "vpn.egress",
            Severity::Critical,
            "traffic is leaving the tunnel",
        );
        let summary = Summary::of(Reach::Running, &[&leak], SETTLED);
        assert_eq!(summary.standing, Standing::Critical);
        assert!(summary.standing.wants_attention());
        assert_eq!(summary.said(), "critical — traffic is leaving the tunnel");
    }

    #[test]
    fn the_summary_takes_the_worst_and_never_an_average() {
        // Two advisories and one broken thing is a broken stack, not a middling one.
        let (a, b, c) = (
            wrong("a", Severity::Advisory, "a note"),
            wrong("b", Severity::Error, "the disk is full"),
            wrong("c", Severity::Advisory, "another note"),
        );
        let summary = Summary::of(Reach::Running, &[&a, &b, &c], SETTLED);
        assert_eq!(summary.standing, Standing::Broken);
        assert_eq!(summary.wanting_attention, 3);
        assert_eq!(summary.said(), "broken — the disk is full, and 2 others");
    }

    #[test]
    fn the_summary_expands_to_the_affected_items_worst_first() {
        // A number an operator cannot act on is not an expansion; the items are.
        let (a, b, c) = (
            wrong("a", Severity::Advisory, "a note"),
            wrong("b", Severity::Error, "the disk is full"),
            wrong("c", Severity::Advisory, "another note"),
        );
        let summary = Summary::of(Reach::Running, &[&a, &b, &c], SETTLED);
        let listed: Vec<(&str, Severity, &str)> = summary
            .affected
            .iter()
            .map(|item| (item.check.as_str(), item.severity, item.summary.as_str()))
            .collect();
        assert_eq!(
            listed,
            vec![
                ("b", Severity::Error, "the disk is full"),
                ("a", Severity::Advisory, "a note"),
                ("c", Severity::Advisory, "another note"),
            ],
            "worst first, and stable among equals so the list does not shuffle"
        );
    }

    #[test]
    fn a_stack_nobody_could_look_at_is_unknown_rather_than_healthy() {
        // Never reported as healthy: an unreachable engine is not evidence of health.
        for reach in [Reach::Unreachable, Reach::Starting] {
            let summary = Summary::of(reach, &[], SETTLED);
            assert_eq!(summary.standing, Standing::Unknown, "{reach:?}");
            assert_ne!(summary.standing, Standing::Healthy);
        }
    }

    #[test]
    fn a_leak_while_starting_is_still_a_leak() {
        // A critical finding outranks every reason to stay quiet.
        let leak = wrong("vpn.egress", Severity::Critical, "leaking");
        assert_eq!(
            Summary::of(Reach::Starting, &[&leak], SETTLED).standing,
            Standing::Critical
        );
    }

    #[test]
    fn a_stack_stopped_on_purpose_is_not_a_failure() {
        let summary = Summary::of(Reach::Stopped, &[], SETTLED);
        assert_eq!(summary.standing, Standing::Stopped);
        assert!(!summary.standing.wants_attention());
        assert_eq!(summary.said(), "stopped");
    }

    #[test]
    fn a_machine_with_nothing_set_up_says_that_rather_than_anything_about_health() {
        let summary = Summary::of(Reach::Unconfigured, &[], SETTLED);
        assert_eq!(summary.standing, Standing::Unconfigured);
        assert_eq!(summary.said(), "not set up");
    }

    #[test]
    fn one_other_thing_reads_as_one_rather_than_ones() {
        let (a, b) = (
            wrong("a", Severity::Error, "the disk is full"),
            wrong("b", Severity::Advisory, "a note"),
        );
        assert_eq!(
            Summary::of(Reach::Running, &[&a, &b], SETTLED).said(),
            "broken — the disk is full, and 1 other"
        );
    }

    // ── Counted once per problem, not once per symptom ────────────

    #[test]
    fn what_failed_because_of_something_else_is_counted_with_it_and_not_again() {
        // A disk that filled and the imports that then failed is one problem.
        let disk = wrong("storage.space", Severity::Error, "the disk is full");
        let (first, second) = (
            caused_by(
                "import.sonarr",
                Severity::Error,
                "sonarr could not import",
                "storage.space",
            ),
            caused_by(
                "import.radarr",
                Severity::Error,
                "radarr could not import",
                "storage.space",
            ),
        );
        let summary = Summary::of(Reach::Running, &[&disk, &first, &second], SETTLED);
        assert_eq!(summary.wanting_attention, 1, "one problem, not three");
        assert_eq!(summary.said(), "broken — the disk is full");
        let listed: Vec<(&str, usize)> = summary
            .affected
            .iter()
            .map(|item| (item.check.as_str(), item.downstream.len()))
            .collect();
        assert_eq!(
            listed,
            vec![("storage.space", 2)],
            "and what it took with it is still readable under it"
        );
    }

    #[test]
    fn a_symptom_whose_cause_is_not_itself_wrong_stands_on_its_own() {
        // The disk recovered but the import is still failing: naming a cause that is
        // no longer raised must not make the symptom disappear.
        let orphan = caused_by(
            "import.sonarr",
            Severity::Error,
            "sonarr could not import",
            "storage.space",
        );
        let summary = Summary::of(Reach::Running, &[&orphan], SETTLED);
        assert_eq!(summary.wanting_attention, 1);
        assert_eq!(summary.said(), "broken — sonarr could not import");
    }

    #[test]
    fn a_worse_thing_is_never_folded_under_a_lesser_one() {
        // The cascade rule must not be able to bury the worst thing on the machine,
        // which is the one outcome the whole summary exists to prevent.
        let gateway = wrong("service.gluetun", Severity::Error, "gluetun stopped");
        let leak = caused_by(
            "vpn.egress",
            Severity::Critical,
            "traffic is leaving the tunnel",
            "service.gluetun",
        );
        let summary = Summary::of(Reach::Running, &[&gateway, &leak], SETTLED);
        assert_eq!(summary.standing, Standing::Critical);
        assert_eq!(summary.wanting_attention, 2, "both, since neither absorbs");
        assert_eq!(
            summary.worst.as_deref(),
            Some("traffic is leaving the tunnel")
        );
    }

    // ── Steady enough to grade a stack by ─────────────────────────

    #[test]
    fn a_fault_that_has_only_just_appeared_counts_immediately() {
        // Holding it back would let a stack with an unverified tunnel read healthy
        // for the first half-minute, and silence claimed as health is the one thing
        // this must never do.
        let fresh = wrong("service.sonarr", Severity::Error, "sonarr stopped");
        assert_eq!(
            Summary::of(Reach::Running, &[&fresh], RAISED).standing,
            Standing::Broken
        );
    }

    #[test]
    fn a_fault_that_keeps_bouncing_is_not_called_fixed_in_the_gaps() {
        // What flaps is a service restarting every few seconds; what makes the
        // summary flap is declaring it fixed between the bounces.
        let mut bouncing = wrong("service.sonarr", Severity::Error, "sonarr stopped");
        bouncing.clear("1100");
        bouncing.raise(
            &Fault::new(Severity::Error, "sonarr stopped", "look at it"),
            "1200",
        );
        bouncing.clear("1300");

        let moments_later = (1300 + STEADY - 1).to_string();
        assert_eq!(
            Summary::of(Reach::Running, &[&bouncing], &moments_later).standing,
            Standing::Broken,
            "it has been coming back; do not call it fixed yet"
        );
        let long_enough = (1300 + STEADY).to_string();
        assert_eq!(
            Summary::of(Reach::Running, &[&bouncing], &long_enough).standing,
            Standing::Healthy,
            "and once it has stayed away, it is genuinely gone"
        );
    }

    #[test]
    fn something_that_cleared_and_never_came_back_is_gone_at_once() {
        // Only a fault with a history of returning is held on to; a one-off that
        // resolved is not kept around for half a minute.
        let mut resolved = wrong("service.sonarr", Severity::Error, "sonarr stopped");
        resolved.clear("1100");
        assert_eq!(
            Summary::of(Reach::Running, &[&resolved], "1101").standing,
            Standing::Healthy
        );
    }

    #[test]
    fn a_clock_problem_cannot_pin_a_resolved_fault_to_the_summary_forever() {
        let mut resolved = wrong("service.sonarr", Severity::Error, "sonarr stopped");
        resolved.clear("1100");
        resolved.raise(
            &Fault::new(Severity::Error, "sonarr stopped", "look at it"),
            "1200",
        );
        resolved.cleared = Some("whenever".to_owned());
        assert_eq!(
            Summary::of(Reach::Running, &[&resolved], "9000").standing,
            Standing::Healthy
        );
    }

    #[test]
    fn a_symptom_is_not_folded_into_a_cause_the_summary_has_stopped_counting() {
        // Folding into a root that is no longer reported would hide both.
        let mut gone_disk = wrong("storage.space", Severity::Error, "the disk is full");
        gone_disk.clear("1100");
        let import = caused_by(
            "import.sonarr",
            Severity::Error,
            "sonarr could not import",
            "storage.space",
        );
        let summary = Summary::of(Reach::Running, &[&gone_disk, &import], SETTLED);
        assert_eq!(summary.wanting_attention, 1);
        assert_eq!(summary.said(), "broken — sonarr could not import");
    }

    #[test]
    fn the_expansion_carries_what_to_do_about_each_thing() {
        let stalled = wrong("queue.stalled", Severity::Warning, "nothing is moving");
        let summary = Summary::of(Reach::Running, &[&stalled], SETTLED);
        let remedies: Vec<Vec<String>> = summary
            .affected
            .iter()
            .map(|item| item.remedies.clone())
            .collect();
        assert_eq!(remedies, vec![vec!["look at it".to_owned()]]);
    }

    #[test]
    fn a_stopped_stack_with_something_wrong_still_reads_as_stopped() {
        // Its containers are down on purpose; a warning raised against them is not a
        // reason to call a deliberately stopped stack degraded.
        let stale = wrong("images.stale", Severity::Warning, "an image is out of date");
        assert_eq!(
            Summary::of(Reach::Stopped, &[&stale], SETTLED).standing,
            Standing::Stopped
        );
    }
}
