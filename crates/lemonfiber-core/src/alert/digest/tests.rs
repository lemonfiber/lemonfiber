use super::{Digest, Moment, Reach, Wants, FLAPPING};
use crate::alert::Appetite;
use crate::condition::{Condition, Fault};
use crate::error::Severity;

/// A condition that has come back `times` times and is wrong now.
fn flapped(check: &str, severity: Severity, times: u32) -> Condition {
    // Its own kind unless a test deliberately shares one, so a digest of
    // several checks is several alerts rather than one group.
    let fault = Fault::new(
        check,
        severity,
        "it broke",
        "nothing that needs it is working",
        "look at it",
    );
    let mut condition = Condition::raised(check, &fault, "1000");
    for n in 0..times {
        condition.clear("1100");
        condition.raise(&fault, &format!("{}", 1200 + n));
    }
    condition
}

/// Nobody has been told anything.
fn untold(_check: &str) -> Option<u32> {
    None
}

#[test]
fn six_things_at_once_are_one_message_worst_first() {
    // Six alerts arriving together read as six emergencies rather than one bad
    // minute, and the worst is what to act on.
    let conditions = vec![
        flapped("b.warn", Severity::Warning, 0),
        flapped("a.critical", Severity::Critical, 0),
        flapped("c.error", Severity::Error, 0),
    ];
    let digest = Digest::of(&conditions, &untold);
    assert_eq!(
        digest
            .alerts
            .iter()
            .map(|a| a.check.as_str())
            .collect::<Vec<_>>(),
        vec!["a.critical", "c.error", "b.warn"]
    );
    assert_eq!(digest.worst(), Some(Severity::Critical));
}

#[test]
fn two_of_one_severity_are_ordered_by_check_so_two_runs_read_alike() {
    // Without a tie-break the order is whatever the source iterated in, and a
    // digest that shuffles between runs is one nobody can scan.
    let second = flapped("b.second", Severity::Error, 0);
    let first = flapped("a.first", Severity::Error, 0);
    let digest = Digest::of([&second, &first], &untold);
    assert_eq!(
        digest
            .alerts
            .iter()
            .map(|a| a.check.as_str())
            .collect::<Vec<_>>(),
        vec!["a.first", "b.second"]
    );
}

#[test]
fn a_digest_says_the_worst_and_counts_the_rest() {
    let critical = flapped("a.critical", Severity::Critical, 0);
    let warning = flapped("b.warn", Severity::Warning, 0);

    let two = Digest::of([&critical, &warning], &untold).headline();
    assert_eq!(two.as_deref(), Some("it broke — started (and 1 other)"));

    let one = Digest::of([&critical], &untold).headline();
    assert_eq!(one.as_deref(), Some("it broke — started"));

    let three = Digest::of(
        [&critical, &warning, &flapped("c.error", Severity::Error, 0)],
        &untold,
    )
    .headline();
    assert_eq!(three.as_deref(), Some("it broke — started (and 2 others)"));
}

#[test]
fn nothing_worth_saying_is_an_empty_digest() {
    let digest = Digest::of(&[], &untold);
    assert!(digest.is_empty());
    assert_eq!(digest.headline(), None);
    assert_eq!(digest.worst(), None);
    assert!(!digest.overrides_quiet());
}

#[test]
fn a_flapping_service_is_reported_as_flapping_rather_than_as_each_flap() {
    // Once is an incident, twice is bad luck; by the third round trip the pattern
    // is the fault, and it has a different remedy from either state.
    let condition = flapped("service.health", Severity::Warning, FLAPPING);
    let digest = Digest::of([&condition], &untold);
    let said = digest.headline().unwrap_or_default();
    assert!(said.contains("come back"), "{said}");
    assert!(said.contains(&FLAPPING.to_string()), "{said}");
}

#[test]
fn a_flapping_service_already_reported_stays_quiet() {
    // The whole point: not one alert per flap, for ever.
    let condition = flapped("service.health", Severity::Warning, FLAPPING + 2);
    let heard = |_: &str| Some(FLAPPING);
    assert!(Digest::of([&condition], &heard).is_empty());
}

#[test]
fn one_critical_carries_the_whole_digest_through_a_quiet_period() {
    // Splitting it would mean the emergency arrives without its context.
    let critical = flapped("a.critical", Severity::Critical, 0);
    let warning = flapped("b.warn", Severity::Warning, 0);
    assert!(Digest::of([&critical, &warning], &untold).overrides_quiet());
    assert!(!Digest::of([&warning], &untold).overrides_quiet());
}

// ── One event about four services ─────────────────────────────

/// A service down, of a shared kind, so several of them are one event.
fn stopped(check: &str, severity: Severity) -> Condition {
    Condition::raised(
        check,
        &Fault::new(
            "service.stopped",
            severity,
            &format!("{check} stopped on its own"),
            "nothing that needs it is working",
            "start it again",
        ),
        "1000",
    )
}

/// A service down, of a shared kind, saying in its own words what its absence
/// costs — so a group speaking for several can be caught speaking for the
/// wrong one.
fn stopped_costing(check: &str, means: &str) -> Condition {
    Condition::raised(
        check,
        &Fault::new(
            "service.stopped",
            Severity::Error,
            &format!("{check} stopped on its own"),
            means,
            "start it again",
        ),
        "1000",
    )
}

#[test]
fn a_grouped_alert_says_what_it_means_in_the_words_of_the_one_that_speaks() {
    // The summary and the meaning are one sentence in two halves. A group that
    // took its event from one service and its consequence from another would
    // describe a stack that does not exist.
    let first = stopped_costing("service.radarr", "no film is being fetched");
    let second = stopped_costing("service.sonarr", "no episode is being fetched");
    let digest = Digest::of([&second, &first], &untold);
    let said: Vec<(&str, &str)> = digest
        .alerts
        .iter()
        .map(|alert| (alert.summary.as_str(), alert.meaning.as_str()))
        .collect();
    assert_eq!(
        said,
        vec![(
            "service.radarr stopped on its own",
            "no film is being fetched"
        )],
        "the earliest by check speaks, in both halves"
    );
}

#[test]
fn a_flapping_alert_still_says_what_the_flapping_costs() {
    // Past the threshold the alert is rewritten to report the pattern, and a
    // rewrite is where the half nobody looks at gets dropped.
    let bouncing = flapped("service.sonarr", Severity::Error, FLAPPING);
    let digest = Digest::of([&bouncing], &untold);
    let means: Vec<&str> = digest
        .alerts
        .iter()
        .map(|alert| alert.meaning.as_str())
        .collect();
    assert_eq!(means, vec!["nothing that needs it is working"]);
}

#[test]
fn four_services_failing_the_same_way_are_one_alert_naming_them_all() {
    // The operator's next action is the same either way, and reading the same
    // sentence four times with a different name in it is how a digest is skimmed.
    let services = ["service.sonarr", "service.radarr", "service.lidarr"];
    let raised: Vec<Condition> = services
        .iter()
        .map(|check| stopped(check, Severity::Error))
        .collect();
    let digest = Digest::of(raised.iter(), &untold);

    assert_eq!(digest.alerts.len(), 1, "one event, three services");
    let listed: Vec<(&str, Vec<&str>)> = digest
        .alerts
        .iter()
        .map(|alert| {
            (
                alert.check.as_str(),
                alert.affected.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        vec![(
            "service.lidarr",
            vec!["service.lidarr", "service.radarr", "service.sonarr"]
        )],
        "the first by check speaks, so two runs of one stack read alike"
    );
    assert_eq!(
        digest.headline().as_deref(),
        Some("service.lidarr stopped on its own — started, and 2 other services")
    );
}

#[test]
fn a_group_is_as_loud_as_the_worst_thing_in_it() {
    // Otherwise whichever service happened to sort first would decide how
    // seriously the operator takes a group containing something critical.
    let ordinary = stopped("service.aaa", Severity::Warning);
    let bad = stopped("service.zzz", Severity::Critical);
    let digest = Digest::of([&ordinary, &bad], &untold);
    assert_eq!(digest.worst(), Some(Severity::Critical));
    assert!(digest.overrides_quiet());
}

#[test]
fn the_same_event_going_two_different_ways_stays_two_alerts() {
    // Two stopped and a third that came back is not one thing that happened.
    let down = stopped("service.aaa", Severity::Error);
    let mut back = stopped("service.zzz", Severity::Error);
    back.clear("1100");
    // Only the one that came back was ever reported, so only its resolution is
    // news; the other is still an onset nobody has heard.
    let heard = |check: &str| (check == "service.zzz").then_some(0);
    let moments: Vec<Moment> = Digest::of([&down, &back], &heard)
        .alerts
        .iter()
        .map(|alert| alert.moment)
        .collect();
    assert_eq!(moments, vec![Moment::Onset, Moment::Resolved]);
}

#[test]
fn an_alert_carries_what_to_do_about_it() {
    // What happened without what to do is a notification, which is a different
    // and worse thing.
    let condition = stopped("service.sonarr", Severity::Error);
    let remedies: Vec<Vec<String>> = Digest::of([&condition], &untold)
        .alerts
        .iter()
        .map(|alert| alert.remedies.clone())
        .collect();
    assert_eq!(remedies, vec![vec!["start it again".to_owned()]]);
}

// ── Not ours to say, and not while stopped ────────────────────

#[test]
fn what_a_service_already_tells_its_own_users_is_never_repeated_here() {
    // A second message from lemonfiber is not an extra courtesy; it is what
    // teaches an operator to mute the channel that also carries the leak.
    let theirs = Condition::raised(
        "request.4231",
        &Fault::new(
            "request.available",
            Severity::Warning,
            "Dune is ready to watch",
            "there is nothing left to wait for",
            "open it",
        ),
        "1000",
    );
    assert!(Digest::of([&theirs], &untold).is_empty());
}

#[test]
fn a_stack_the_operator_stopped_says_nothing_about_being_down() {
    // Every service being down is what was asked for. Reporting it as a fault
    // teaches the operator that stopping the stack means a page of alerts.
    let down = stopped("service.sonarr", Severity::Error);
    assert!(Digest::reached(Reach::Stopped, [&down], &untold).is_empty());
    assert!(!Digest::reached(Reach::Running, [&down], &untold).is_empty());
}

#[test]
fn an_engine_nobody_could_reach_is_not_a_stack_somebody_turned_off() {
    // Suppressing on "not running" rather than "stopped on purpose" would go
    // quiet exactly when the machine stopped answering.
    let down = stopped("service.sonarr", Severity::Error);
    for reach in [Reach::Unreachable, Reach::Starting, Reach::Unconfigured] {
        assert!(
            !Digest::reached(reach, [&down], &untold).is_empty(),
            "{reach:?}"
        );
    }
}

#[test]
fn a_channel_that_will_not_take_deliveries_is_reported_even_while_stopped() {
    // It is wrong whatever the stack is doing, and it is the reason the operator
    // may not have heard anything else.
    let refusing = Condition::raised(
        "notify.channel.discord",
        &Fault::new(
            "notify.channel.refused",
            Severity::Warning,
            "discord would not take it",
            "alerts are not reaching it",
            "check the channel's configuration",
        ),
        "1000",
    );
    assert!(!Digest::reached(Reach::Stopped, [&refusing], &untold).is_empty());
}

// ── Only what was asked for ───────────────────────────────────

/// A finished download, which is advisory and a completion.
fn finished() -> Condition {
    Condition::raised(
        "download.ubuntu-iso",
        &Fault::new(
            "download.completed",
            Severity::Advisory,
            "Ubuntu.iso finished",
            "it is on the disk now",
            "nothing to do",
        ),
        "1000",
    )
}

#[test]
fn the_quiet_preset_hears_problems_and_nothing_else() {
    let done = finished();
    let broken = stopped("service.sonarr", Severity::Error);
    let quiet = Wants::preset(Appetite::ProblemsOnly);
    let kinds: Vec<String> = Digest::wanted(Reach::Running, &quiet, [&done, &broken], &untold)
        .alerts
        .iter()
        .map(|alert| alert.kind.clone())
        .collect();
    assert_eq!(kinds, vec!["service.stopped".to_owned()]);
}

#[test]
fn asking_for_completions_hears_them_without_hearing_everything() {
    let done = finished();
    let notice = Condition::raised(
        "update.stack",
        &Fault::new(
            "update.available",
            Severity::Advisory,
            "a newer stack is available",
            "this one goes on working meanwhile",
            "upgrade when convenient",
        ),
        "1000",
    );
    let wants = Wants::preset(Appetite::WithCompletions);
    let kinds: Vec<String> = Digest::wanted(Reach::Running, &wants, [&done, &notice], &untold)
        .alerts
        .iter()
        .map(|alert| alert.kind.clone())
        .collect();
    assert_eq!(kinds, vec!["download.completed".to_owned()]);
}

#[test]
fn one_event_switched_on_arrives_without_the_rest_of_its_class() {
    // The preset is a starting point, not a ceiling.
    let done = finished();
    let mut wants = Wants::preset(Appetite::ProblemsOnly);
    wants.set("download.completed", true);
    assert!(!Digest::wanted(Reach::Running, &wants, [&done], &untold).is_empty());
}

#[test]
fn something_wrong_is_heard_however_quiet_the_operator_asked_to_be() {
    // The floor under every preset: a leak is not something to be opted out of
    // by choosing the quiet option at setup.
    let leak = Condition::raised(
        "vpn.egress",
        &Fault::new(
            "vpn.egress.leaking",
            Severity::Critical,
            "traffic is leaving the tunnel",
            "this connection's address is visible to every peer",
            "stop the download client",
        ),
        "1000",
    );
    let quiet = Wants::preset(Appetite::ProblemsOnly);
    assert!(!Digest::wanted(Reach::Running, &quiet, [&leak], &untold).is_empty());
}

#[test]
fn a_resolution_arrives_on_the_same_terms_as_the_onset_that_was_heard() {
    // An operator told a disk filled up and never told it was resolved goes on
    // believing it.
    let mut done = finished();
    done.clear("1100");
    let mut wants = Wants::preset(Appetite::ProblemsOnly);
    wants.set("download.completed", true);
    let heard = |_: &str| Some(0);
    let moments: Vec<Moment> = Digest::wanted(Reach::Running, &wants, [&done], &heard)
        .alerts
        .iter()
        .map(|alert| alert.moment)
        .collect();
    assert_eq!(moments, vec![Moment::Resolved]);
}
