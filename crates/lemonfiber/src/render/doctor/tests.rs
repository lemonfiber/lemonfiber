use super::*;
use crate::render::fixtures::*;
use lemonfiber_core::doctor::{Category, Finding, Overall, Verdict};
use lemonfiber_core::error::{Code, Problem, Remedy, Severity};

/// A finding that failed, for the tests that are about what is shown beside one.
fn a_failing_finding() -> Finding {
    Finding {
        check: "services.sonarr".to_owned(),
        category: Category::Services,
        title: "Sonarr answers".to_owned(),
        service: Some("sonarr".to_owned()),
        caused_by: None,
        said: None,
        verdict: Verdict::Fail(a_problem()),
        origin: lemonfiber_core::origin::Origin::Bundled,
    }
}

/// The point of the requirement: the evidence is at the failure, not somewhere
/// the operator has to go and look for it.
#[test]
fn a_failing_finding_quotes_what_the_service_said() {
    let report = DoctorReport {
        overall: Overall::Broken,
        findings: vec![Finding {
            said: Some("Database is locked\nRetrying in 30s\n".to_owned()),
            ..a_failing_finding()
        }],
    };

    let text = diagnosis(&report).text();
    assert!(text.contains("it said:"), "{text}");
    assert!(text.contains("Database is locked"), "{text}");
    assert!(
        text.contains("        Retrying in 30s"),
        "every line is indented under the finding, not just the first: {text}"
    );
}

/// A plugin's check is marked as the plugin's beside its title, and the page says
/// what an unmarked row is; a page of only this build's own marks nothing and says
/// nothing about marks.
#[test]
fn a_plugin_s_check_says_whose_it_is_beside_its_title() {
    let theirs = Finding {
        title: "Komga libraries".to_owned(),
        origin: lemonfiber_core::origin::Origin::Plugin {
            named: "komga".to_owned(),
        },
        ..a_failing_finding()
    };
    let unknown = Finding {
        title: "A stray row".to_owned(),
        origin: lemonfiber_core::origin::Origin::Unknown {
            why: "nothing said".to_owned(),
        },
        ..a_failing_finding()
    };
    let mixed = diagnosis(&DoctorReport {
        overall: Overall::Broken,
        findings: vec![a_failing_finding(), theirs, unknown],
    })
    .text();
    assert!(
        mixed.contains("✗ Komga libraries (from plugin komga)"),
        "{mixed}"
    );
    assert!(mixed.contains("✗ A stray row (from unknown)"), "{mixed}");
    assert!(mixed.contains("✗ Sonarr answers   "), "{mixed}");
    assert!(mixed.contains("every other is"), "{mixed}");

    let ours = diagnosis(&DoctorReport {
        overall: Overall::Broken,
        findings: vec![a_failing_finding()],
    })
    .text();
    assert!(!ours.contains("every other is"), "{ours}");
}

/// A heading with nothing under it promises evidence that is not there.
#[test]
fn a_finding_with_nothing_said_shows_no_heading() {
    for said in [None, Some(String::new()), Some("   \n".to_owned())] {
        let report = DoctorReport {
            overall: Overall::Broken,
            findings: vec![Finding {
                said,
                ..a_failing_finding()
            }],
        };
        assert!(!diagnosis(&report).text().contains("it said:"));
    }
}

/// The classification an operator can act on. A finding lemonfiber can put right
/// itself says so and names the command; the ones only they can act on already say
/// where to go in their own remedies, so a label there would be noise.
#[test]
fn a_finding_lemonfiber_can_mend_says_so_and_names_the_command() {
    use lemonfiber_core::error::State;

    let mendable = remedies(&a_problem().in_state(State::Remediable)).text();
    assert!(
        mendable.contains("can put this one right for you"),
        "{mendable}"
    );
    assert!(mendable.contains("lemonfiber doctor --fix"), "{mendable}");

    let theirs = remedies(&a_problem()).text();
    assert!(!theirs.contains("can put this one right"), "{theirs}");
}

#[test]
fn every_verdict_reads_with_its_own_mark() {
    let findings = vec![
        Finding {
            check: "a".to_owned(),
            category: Category::Storage,
            title: "noted".to_owned(),
            service: None,
            caused_by: None,
            said: None,
            verdict: Verdict::Pass {
                note: Some("plenty of room".to_owned()),
            },
            origin: lemonfiber_core::origin::Origin::Bundled,
        },
        Finding {
            check: "b".to_owned(),
            category: Category::Storage,
            title: "bare".to_owned(),
            service: None,
            caused_by: None,
            said: None,
            verdict: Verdict::Pass { note: None },
            origin: lemonfiber_core::origin::Origin::Bundled,
        },
        Finding {
            check: "c".to_owned(),
            category: Category::Vpn,
            title: "warned".to_owned(),
            service: None,
            caused_by: None,
            said: None,
            verdict: Verdict::Warn(a_problem()),
            origin: lemonfiber_core::origin::Origin::Bundled,
        },
        Finding {
            check: "d".to_owned(),
            category: Category::Vpn,
            title: "failed".to_owned(),
            service: None,
            caused_by: None,
            said: None,
            verdict: Verdict::Fail(a_problem()),
            origin: lemonfiber_core::origin::Origin::Bundled,
        },
        Finding {
            check: "e".to_owned(),
            category: Category::Network,
            title: "unproven".to_owned(),
            service: None,
            caused_by: None,
            said: None,
            verdict: Verdict::Unverified {
                reason: "nothing answered".to_owned(),
                remedy: Remedy::new("start it").with_detail("compose up"),
            },
            origin: lemonfiber_core::origin::Origin::Bundled,
        },
        Finding {
            check: "f".to_owned(),
            category: Category::Network,
            title: "passed over".to_owned(),
            service: None,
            caused_by: None,
            said: None,
            verdict: Verdict::Skipped {
                reason: "not applicable".to_owned(),
            },
            origin: lemonfiber_core::origin::Origin::Bundled,
        },
    ];
    let report = DoctorReport {
        overall: Overall::Degraded,
        findings,
    };
    let text = diagnosis(&report).text();
    assert!(text.contains("✓ noted   plenty of room"));
    assert!(text.contains("✓ bare"));
    assert!(text.contains("! warned   it broke"));
    assert!(text.contains("✗ failed   it broke"));
    assert!(text.contains("? unproven   UNVERIFIED"));
    assert!(text.contains("→ start it"));
    assert!(text.contains("compose up"));
    assert!(text.contains("– passed over   skipped: not applicable"));
    assert!(text.contains("degraded — working, with warnings"));
}

#[test]
fn an_unverified_finding_without_detail_still_carries_its_remedy() {
    let report = DoctorReport {
        overall: Overall::Unknown,
        findings: vec![Finding {
            check: "a".to_owned(),
            category: Category::Config,
            title: "unproven".to_owned(),
            service: None,
            caused_by: None,
            said: None,
            verdict: Verdict::Unverified {
                reason: "nothing answered".to_owned(),
                remedy: Remedy::new("start it"),
            },
            origin: lemonfiber_core::origin::Origin::Bundled,
        }],
    };
    assert!(diagnosis(&report).text().contains("→ start it"));
}

#[test]
fn a_remedy_without_detail_prints_only_its_action() {
    let problem = Problem::new(
        Code::new("TEST"),
        Severity::Warning,
        "it broke",
        "nothing imports",
        Remedy::new("restart it"),
    );
    let text = remedies(&problem).text();
    assert!(text.contains("nothing imports"));
    assert!(text.contains("→ restart it"));
}

#[test]
fn every_overall_verdict_reads_as_a_sentence() {
    for verdict in [
        Overall::Healthy,
        Overall::Degraded,
        Overall::Broken,
        Overall::Unknown,
    ] {
        assert!(overall(verdict).contains('—'));
    }
}

#[test]
fn an_answered_choice_stops_leading_without_disappearing() {
    // The whole point of suppressing rather than removing: it is still on
    // screen, still saying what it costs, and no longer putting a remedy for
    // something the operator has already decided against.
    let report = DoctorReport {
        overall: Overall::Degraded,
        findings: vec![Finding {
            check: "vpn.unprotected".to_owned(),
            category: Category::Vpn,
            title: "Torrent traffic is contained".to_owned(),
            service: None,
            caused_by: None,
            said: None,
            verdict: Verdict::Warn(a_problem().in_state(State::Suppressed)),
            origin: lemonfiber_core::origin::Origin::Bundled,
        }],
    };
    let text = diagnosis(&report).text();
    assert!(text.contains("· Torrent traffic is contained   it broke (answered)"));
    assert!(!text.contains("→ "), "the remedy is not put again: {text}");
}
