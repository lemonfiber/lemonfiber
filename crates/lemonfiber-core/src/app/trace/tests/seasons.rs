//! A series followed episode by episode, season by season.

use super::*;

/// The same, recorded against one part — an episode's own history event.
fn part_event(outcome: Outcome, part: i64) -> TraceEvent {
    TraceEvent {
        part: Some(part),
        ..event(outcome)
    }
}

#[test]
fn a_series_is_reported_season_by_season_not_by_one_episode_landing() {
    // The gap this closes: the history shows an import, so the item as a whole is
    // "imported" — which on its own reads as done while half the show is missing.
    let mut fragments = frags(vec![event(Outcome::Imported)], Vec::new(), None);
    fragments.parts = vec![
        episode(11, 1, 1, true, true),
        episode(12, 1, 2, true, false),
        episode(21, 2, 1, true, true),
        episode(22, 2, 2, true, true),
    ];
    let report = assemble("Sonarr", "The Expanse", true, fragments);
    assert_eq!(report.furthest, Stage::Imported);
    let coverage = report.coverage.unwrap_or_default();
    assert_eq!((coverage.have, coverage.wanted), (3, 4));
    assert!(!coverage.complete());
    // Season two is whole; season one is the one an operator would go and look at.
    let whole: Vec<bool> = coverage
        .seasons
        .iter()
        .map(crate::trace::SeasonCoverage::complete)
        .collect();
    assert_eq!(whole, vec![false, true]);
    assert_eq!(outstanding(&coverage), vec![(1, 2, Stage::Monitored)]);
}

#[test]
fn an_episode_the_queue_is_downloading_is_not_a_stalled_grab() {
    // The join the coverage exists for. On the episode's own record it is grabbed and
    // nothing more, which reads as "the download client never took it" — but the queue
    // says it is downloading right now. Without the lift, every episode in flight
    // would report as a fault.
    let mut fragments = frags(
        vec![part_event(Outcome::Grabbed, 12)],
        vec![QueueItem {
            part: Some(12),
            stage: Stage::Downloading,
            stuck: false,
        }],
        None,
    );
    fragments.parts = vec![
        episode(11, 1, 1, true, true),
        episode(12, 1, 2, true, false),
    ];
    let report = assemble("Sonarr", "The Expanse", true, fragments);
    let coverage = report.coverage.unwrap_or_default();
    assert_eq!(outstanding(&coverage), vec![(1, 2, Stage::Downloading)]);
    // Downloading is work in progress, so it carries no stall reason at all.
    assert_eq!(Stage::Downloading.stall(), None);
}

#[test]
fn a_grabbed_episode_the_queue_never_took_keeps_its_stall() {
    // The other side of the same join: grabbed, and the queue holds nothing for it.
    let mut fragments = frags(vec![part_event(Outcome::Grabbed, 12)], Vec::new(), None);
    fragments.parts = vec![episode(12, 1, 2, true, false)];
    let report = assemble("Sonarr", "The Expanse", true, fragments);
    let coverage = report.coverage.unwrap_or_default();
    assert_eq!(outstanding(&coverage), vec![(1, 2, Stage::Grabbed)]);
    assert!(Stage::Grabbed
        .stall()
        .is_some_and(|reason| reason.contains("download client never took it")));
}

#[test]
fn an_item_with_no_parts_reports_no_coverage() {
    // A film is the whole item — there is nothing to aggregate, so no coverage is
    // claimed rather than an empty one implying a series with nothing in it.
    let report = assemble(
        "Radarr",
        "Dune",
        true,
        frags(vec![event(Outcome::Imported)], Vec::new(), None),
    );
    assert_eq!(report.coverage, None);
}

#[test]
fn unreadable_episodes_are_not_read_as_a_series_with_nothing_in_it() {
    let mut fragments = frags(Vec::new(), Vec::new(), None);
    fragments.reads.parts = false;
    let report = assemble("Sonarr", "The Expanse", true, fragments);
    assert_eq!(report.coverage, None);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("episodes could not be read")));
}

#[test]
fn a_queue_record_behind_the_history_is_not_shown_as_the_current_step() {
    // An item already imported, with a queue record still saying downloading — a
    // leftover the service has not cleared. Adding it would read as the item having
    // gone backwards, so the queue only shows where it carries the item forward.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(
            vec![event(Outcome::Imported)],
            queued(Stage::Downloading, false),
            None,
        ),
    );
    assert_eq!(report.furthest, Stage::Imported);
    assert_eq!(
        report.stages.last().map(|stage| stage.stage),
        Some(Stage::Imported)
    );
}

#[tokio::test]
async fn an_unreadable_episode_listing_leaves_the_rest_of_the_trace_standing() {
    // The episodes will not read, so there is no coverage to report — but everything
    // the other services did answer still stands, and the gap is named.
    let context = ctx_with(&Fake::with_episodes(
        r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
        EMPTY_QUEUE,
        "not json",
    ));
    let report = trace(&context, "expanse", None, false)
        .await
        .unwrap_or_default();
    assert!(report.matched);
    assert_eq!(report.coverage, None);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("episodes could not be read")));
}

#[tokio::test]
async fn tracing_a_series_reads_its_episodes_into_coverage() {
    let context = ctx_with(&Fake::with_episodes(
        r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
        EMPTY_QUEUE,
        r#"[
            {"id":11,"seasonNumber":1,"episodeNumber":1,"monitored":true,"hasFile":true},
            {"id":12,"seasonNumber":1,"episodeNumber":2,"monitored":true,"hasFile":false},
            {"id":13,"seasonNumber":0,"episodeNumber":1,"monitored":false,"hasFile":false}
        ]"#,
    ));
    let report = trace(&context, "expanse", None, false)
        .await
        .unwrap_or_default();
    let coverage = report.coverage.unwrap_or_default();
    // The special nobody asked for is counted apart, never dragging the denominator
    // to three and reading as a fault to chase.
    assert_eq!((coverage.have, coverage.wanted), (1, 2));
    assert_eq!(coverage.unmonitored, 1);
}
