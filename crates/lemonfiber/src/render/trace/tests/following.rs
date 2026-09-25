//! A trace: its stages, its history and its season coverage.

use super::*;

#[test]
fn a_trace_link_names_the_term_the_trace_searches_by() {
    assert!(trace_link("The Expanse").contains("trace 'The Expanse'"));
}

/// This line is a command an operator copies, and the title in it is an \*arr's or
/// Overseerr's rather than ours.
///
/// A title carrying the quote the line was built with closed it early, and everything
/// after that was read by the shell as further arguments. What follows the quote is
/// the operator's own shell, so the failure is not that the trace finds nothing.
#[test]
fn a_title_cannot_close_the_quoting_the_command_is_written_with() {
    for title in [
        r#"Say "Anything""#,
        "It's Always Sunny",
        "Airplane! (1980)",
        "$HOME `whoami`",
        r"Back\Slash",
        "The Expanse",
    ] {
        let quoted = one_argument(title);
        let link = trace_link(title);
        assert_eq!(
            link.split_once("trace ").map(|(_, said)| said),
            Some(quoted.as_str()),
            "{link}"
        );
        assert_eq!(unquoted(&quoted), title, "{link}");
    }
}

/// What a shell hands over, given one single-quoted word.
///
/// Read back rather than compared against a written-out expectation: the claim is
/// that the trace is given the title, and an expectation spelled by hand would be
/// this test agreeing with whatever the quoting happened to produce.
fn unquoted(said: &str) -> String {
    let mut term = String::new();
    let mut quoting = false;
    let mut marks = said.chars();
    while let Some(mark) = marks.next() {
        match mark {
            '\'' => quoting = !quoting,
            '\\' if !quoting => term.extend(marks.next()),
            other => term.push(other),
        }
    }
    term
}

#[test]
fn an_unmatched_trace_says_nobody_asked_for_it() {
    let report = TraceReport {
        item: "Nothing".to_owned(),
        matched: false,
        stall: Some("nobody has asked for it".to_owned()),
        ..TraceReport::default()
    };
    let text = trace(&report).text();
    assert!(text.contains("nobody has asked for it"));
    // The stage box belongs to a matched item; an unmatched one stops here.
    assert!(!text.contains("history events per service"));
    // And one with no reason at all still renders its name.
    let bare = TraceReport {
        item: "Nothing".to_owned(),
        matched: false,
        ..TraceReport::default()
    };
    assert_eq!(trace(&bare).text(), "Nothing");
}

#[test]
fn a_trace_shows_its_stages_stall_history_and_horizon() {
    let report = TraceReport {
        stages: vec![
            TraceStage {
                stage: Stage::Monitored,
                service: "Sonarr".to_owned(),
                at: None,
            },
            TraceStage {
                stage: Stage::Grabbed,
                service: "Sonarr".to_owned(),
                at: Some("2026-01-01".to_owned()),
            },
        ],
        stall: Some("the download client never took it".to_owned()),
        history: vec![
            TraceMoment {
                outcome: TraceOutcome::Grabbed,
                at: "2026-01-01".to_owned(),
            },
            TraceMoment {
                outcome: TraceOutcome::DownloadFailed,
                at: "2026-01-02".to_owned(),
            },
        ],
        findings: vec!["the queue could not be read".to_owned()],
        confidence: Confidence::Uncertain,
        ..a_trace()
    };
    let text = trace(&report).text();
    assert!(text.contains("✓ monitored   Sonarr"));
    assert!(text.contains("✓ grabbed   Sonarr   2026-01-01"));
    assert!(text.contains("✗ stopped:"));
    assert!(text.contains("history:"));
    assert!(text.contains("! the queue could not be read"));
    assert!(text.contains("~ matched to the library by title"));
    assert!(text.contains(&format!("most recent {HISTORY_HORIZON} history events")));
}

#[test]
fn a_single_clean_grab_does_not_repeat_itself_as_history() {
    let report = TraceReport {
        history: vec![TraceMoment {
            outcome: TraceOutcome::Grabbed,
            at: "2026-01-01".to_owned(),
        }],
        ..a_trace()
    };
    assert!(!trace(&report).text().contains("history:"));
}

#[test]
fn a_season_rollup_names_what_is_outstanding_and_what_nobody_asked_for() {
    let coverage = Coverage::of(vec![
        Part {
            season: 1,
            number: 1,
            title: "one".to_owned(),
            stage: Stage::Imported,
        },
        Part {
            season: 1,
            number: 2,
            title: "two".to_owned(),
            stage: Stage::Monitored,
        },
        Part {
            season: 1,
            number: 3,
            title: "three".to_owned(),
            stage: Stage::NotMonitored,
        },
        Part {
            season: 2,
            number: 1,
            title: "four".to_owned(),
            stage: Stage::Imported,
        },
    ]);
    let text = seasons(&coverage).text();
    assert!(text.contains("2 of 3 episode(s) here"));
    assert!(text.contains("season 1   1 of 2"));
    assert!(text.contains("(1 more not asked for)"));
    assert!(text.contains("season 2   1 of 1   complete"));
    // The outstanding episode carries the reason it stopped, not just its number.
    assert!(text.contains("S01E02   monitored, but nothing has been grabbed"));
}

#[test]
fn a_season_nobody_asked_for_reads_as_that_rather_than_as_a_fault() {
    let coverage = Coverage::of(vec![
        Part {
            season: 0,
            number: 1,
            title: "special".to_owned(),
            stage: Stage::NotMonitored,
        },
        Part {
            season: 1,
            number: 1,
            title: "one".to_owned(),
            stage: Stage::Imported,
        },
    ]);
    assert!(seasons(&coverage)
        .text()
        .contains("specials   1 not asked for"));
    // And a series with nothing wanted at all says so instead of "0 of 0".
    let none = Coverage::of(vec![Part {
        season: 1,
        number: 1,
        title: "one".to_owned(),
        stage: Stage::NotMonitored,
    }]);
    assert!(seasons(&none).text().contains("no episode(s) asked for"));
}

#[test]
fn an_outstanding_episode_in_progress_reads_as_its_stage() {
    // Downloading carries no stall reason, so the stage's own label stands in.
    let coverage = Coverage::of(vec![Part {
        season: 1,
        number: 1,
        title: "one".to_owned(),
        stage: Stage::Downloading,
    }]);
    assert!(seasons(&coverage).text().contains("S01E01   downloading"));
}

#[test]
fn a_trace_folds_in_its_coverage() {
    let report = TraceReport {
        coverage: Some(Coverage::of(vec![Part {
            season: 1,
            number: 1,
            title: "one".to_owned(),
            stage: Stage::Imported,
        }])),
        ..a_trace()
    };
    assert!(trace(&report).text().contains("1 of 1 episode(s) here"));
}
