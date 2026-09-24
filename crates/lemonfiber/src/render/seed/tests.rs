use super::*;
use crate::render::fixtures::*;
use lemonfiber_core::seed::{
    Assessment as SeedAssessment, Report as SeedReport, Severity as SeedSeverity,
    State as SeedState, Wiring,
};

#[test]
fn every_seed_state_says_what_became_of_the_connection() {
    let report = seed_report(vec![
        wiring("a", SeedState::Wired),
        wiring("b", SeedState::AlreadyWired),
        wiring("c", SeedState::Drifted),
        wiring("d", SeedState::Adopted),
        wiring("e", SeedState::Unmanaged),
        wiring(
            "f",
            SeedState::Observed {
                reason: "I tune this one by hand every season".to_owned(),
            },
        ),
        wiring("l", SeedState::Stale),
        wiring(
            "g",
            SeedState::Conflicted {
                yours: Some("mine".to_owned()),
                ours: "ours".to_owned(),
            },
        ),
        wiring(
            "h",
            SeedState::Conflicted {
                yours: None,
                ours: "ours".to_owned(),
            },
        ),
        wiring(
            "i",
            SeedState::Skipped {
                reason: "not up".to_owned(),
            },
        ),
        wiring(
            "j",
            SeedState::Failed {
                detail: "refused".to_owned(),
            },
        ),
        wiring(
            "k",
            SeedState::Refused {
                reason: "two arrs".to_owned(),
            },
        ),
        wiring(
            "l",
            SeedState::WouldWire {
                yours: None,
                ours: Some("/data/media/tv".to_owned()),
            },
        ),
        wiring(
            "m",
            SeedState::WouldWire {
                yours: Some("tv-sonarr".to_owned()),
                ours: Some("lemonfiber".to_owned()),
            },
        ),
        wiring(
            "n",
            SeedState::WouldWire {
                yours: None,
                ours: None,
            },
        ),
        wiring("o", SeedState::WouldAdopt),
    ]);
    let text = seeding(&report).text();
    for phrase in [
        "wired",
        "already wired",
        "left as you set it",
        "yours, adopted",
        "found already set",
        "yours for now",
        "you declared it unmanaged",
        "I tune this one by hand every season",
        "conflict — both you and the default changed it",
        "you set “mine”",
        "you cleared it",
        "skipped",
        "refused",
        "would be set to “/data/media/tv”",
        "would be changed from “tv-sonarr” to “lemonfiber”",
        "would be set to a newly generated value",
        "would be adopted",
    ] {
        assert!(text.contains(phrase), "missing {phrase}");
    }
}

/// A rehearsal's last lines are the two an operator has to read: what a real run
/// would still have to do, and that this one did none of it.
#[test]
fn a_rehearsed_pass_says_it_wrote_nothing_and_what_running_it_for_real_would_take() {
    let waiting = SeedReport {
        wirings: vec![wiring(
            "a",
            SeedState::WouldWire {
                yours: None,
                ours: Some("http://sonarr:8989".to_owned()),
            },
        )],
        assessment: SeedAssessment::Assessed,
        unsupported: Vec::new(),
        rehearsed: true,
    };
    let text = seeding(&waiting).text();
    assert!(text.contains("1 left to wire — run it again without --dry-run."));
    assert!(text.contains("Nothing was written."));
    assert!(
        !text.contains("run seed again once ready"),
        "a rehearsal that tells them to run it again has told them it ran: {text}"
    );

    let settled = SeedReport {
        wirings: vec![wiring("a", SeedState::AlreadyWired)],
        assessment: SeedAssessment::Assessed,
        unsupported: Vec::new(),
        rehearsed: true,
    };
    let done = seeding(&settled).text();
    assert!(done.contains("a real run would change nothing"), "{done}");
    assert!(done.contains("Nothing was written."), "{done}");
}

/// A service the pass could not speak to is named under the connections, with why,
/// rather than being absent from a report that otherwise reads as complete.
#[test]
fn a_service_nothing_could_be_wired_into_is_named_with_why() {
    let report = SeedReport {
        wirings: vec![wiring("a", SeedState::Wired)],
        assessment: SeedAssessment::Assessed,
        unsupported: vec![lemonfiber_core::model::UnsupportedReport {
            what: "bookish".to_owned(),
            because: "lemonfiber does not speak this service's API yet".to_owned(),
        }],
        rehearsed: false,
    };
    let text = seeding(&report).text();
    assert!(text.contains("cannot speak to them"), "{text}");
    assert!(
        text.contains("bookish — lemonfiber does not speak"),
        "{text}"
    );
}

/// And a stack with nothing of the sort hears nothing about it.
#[test]
fn a_pass_that_could_speak_to_everything_says_nothing_about_it() {
    let report = seed_report(vec![wiring("a", SeedState::Wired)]);
    assert!(!seeding(&report).text().contains("cannot speak to them"));
}

#[test]
fn a_drift_that_breaks_the_stack_is_raised_beneath_its_line() {
    let report = seed_report(vec![Wiring {
        connection: "root folder".to_owned(),
        state: SeedState::Drifted,
        severity: SeedSeverity::Warning {
            breakage: "the path does not exist".to_owned(),
            remediation: "create it".to_owned(),
        },
    }]);
    let text = seeding(&report).text();
    assert!(text.contains("! the path does not exist"));
    assert!(text.contains("→ create it"));
    assert!(text.contains("1 drifted in a way that breaks the stack"));
}

#[test]
fn what_a_seed_still_owes_is_the_last_thing_said() {
    // Everything settled.
    assert!(seeding(&seed_report(vec![wiring("a", SeedState::Wired)]))
        .text()
        .contains("Everything is wired."));
    // Outstanding but nothing blocked.
    let waiting = seed_report(vec![wiring(
        "a",
        SeedState::Skipped {
            reason: "not up".to_owned(),
        },
    )]);
    assert!(seeding(&waiting).text().contains("1 left to wire"));
    // Everything outstanding is blocked.
    let blocked = seed_report(vec![wiring(
        "a",
        SeedState::Refused {
            reason: "two arrs".to_owned(),
        },
    )]);
    assert!(seeding(&blocked).text().contains("1 to resolve"));
    // A mix of the two.
    let mixed = seed_report(vec![
        wiring(
            "a",
            SeedState::Refused {
                reason: "two arrs".to_owned(),
            },
        ),
        wiring(
            "b",
            SeedState::Skipped {
                reason: "not up".to_owned(),
            },
        ),
    ]);
    assert!(seeding(&mixed)
        .text()
        .contains("2 left: 1 to wire once ready"));
}

#[test]
fn a_lost_baseline_says_drift_could_not_be_assessed() {
    let report = SeedReport {
        wirings: vec![wiring("a", SeedState::Wired)],
        assessment: SeedAssessment::Unassessable,
        rehearsed: false,
        unsupported: Vec::new(),
    };
    assert!(seeding(&report).text().contains("could not be read"));
}
