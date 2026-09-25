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
    Condition::raised(
        check,
        &Fault::new(
            check,
            severity,
            summary,
            "nothing that needs it is working",
            "look at it",
        ),
        RAISED,
    )
}

/// One thing wrong, downstream of another.
fn caused_by(check: &str, severity: Severity, summary: &str, cause: &str) -> Condition {
    Condition::raised(
        check,
        &Fault::new(
            check,
            severity,
            summary,
            "nothing that needs it is working",
            "look at it",
        )
        .caused_by(cause),
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
        &Fault::new(
            "service.stopped",
            Severity::Error,
            "sonarr stopped",
            "nothing that needs it is working",
            "look at it",
        ),
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
        &Fault::new(
            "service.stopped",
            Severity::Error,
            "sonarr stopped",
            "nothing that needs it is working",
            "look at it",
        ),
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
fn the_expansion_carries_what_each_thing_costs_as_well_as_what_to_do() {
    // The line expands so the operator has something to act on. An item that
    // states the event and the fix and not what stands between them hands back
    // the judgement the summary exists to make for them.
    let stalled = wrong("queue.stalled", Severity::Warning, "nothing is moving");
    let summary = Summary::of(Reach::Running, &[&stalled], SETTLED);
    let read: Vec<(&str, &str, usize)> = summary
        .affected
        .iter()
        .map(|item| {
            (
                item.summary.as_str(),
                item.meaning.as_str(),
                item.remedies.len(),
            )
        })
        .collect();
    assert_eq!(
        read,
        vec![("nothing is moving", "nothing that needs it is working", 1)]
    );
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
