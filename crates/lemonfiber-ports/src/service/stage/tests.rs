use super::{Confidence, Coverage, Outcome, Part, Stage};

#[test]
fn the_stages_are_declared_in_pipeline_order() {
    // A later stage compares greater, so "the furthest reached" is a max — and the
    // ALL array is that same order for a surface to walk.
    let mut sorted = Stage::ALL;
    sorted.sort_unstable();
    assert_eq!(sorted, Stage::ALL);
    assert!(Stage::NotMonitored < Stage::Available);
    assert!(Stage::Found < Stage::Grabbed);
}

#[test]
fn every_stage_has_a_plain_label() {
    for stage in Stage::ALL {
        let label = stage.label();
        assert!(!label.is_empty());
        // Plain words a household reads, never an internal identifier.
        assert!(!label.contains('_'));
        assert!(label.chars().all(|c| c.is_ascii_lowercase() || c == '-'));
    }
}

#[test]
fn in_progress_stages_are_the_transient_ones() {
    assert!(Stage::Searching.in_progress());
    assert!(Stage::Downloading.in_progress());
    assert!(Stage::Importing.in_progress());
    assert!(!Stage::Monitored.in_progress());
    assert!(!Stage::Available.in_progress());
}

#[test]
fn a_resting_pre_terminal_stage_says_why_it_stopped() {
    // The R5 distinctions: each place an item can silently rest carries its own
    // reason, so "nothing happened" is never left ambiguous.
    for stage in [
        Stage::NotMonitored,
        Stage::Monitored,
        Stage::Found,
        Stage::Grabbed,
        Stage::Downloaded,
        Stage::Imported,
    ] {
        assert!(stage.stall().is_some(), "{} has no reason", stage.label());
    }
}

#[test]
fn success_and_work_in_progress_are_not_stalls() {
    assert_eq!(Stage::Available.stall(), None);
    assert_eq!(Stage::Searching.stall(), None);
    assert_eq!(Stage::Downloading.stall(), None);
    assert_eq!(Stage::Importing.stall(), None);
}

#[test]
fn the_never_found_and_never_grabbed_reasons_are_distinct() {
    // The two most-confused cases must not read the same: nothing grabbed yet is a
    // different problem from releases the quality in force rejects.
    assert_ne!(Stage::Monitored.stall(), Stage::Found.stall());
}

#[test]
fn resting_at_monitored_claims_no_cause_it_cannot_know() {
    // A monitored item with nothing grabbed for it may have had no search run, or a
    // search that came back empty, or a search whose every release the quality in
    // force rejected. This stage alone tells none of the three apart, so it names
    // none of them: the indexers, the quality preset and any search are all absent
    // from what it says.
    let resting = Stage::Monitored.stall().unwrap_or_default();
    for claimed in ["indexer", "quality", "search"] {
        assert!(!resting.contains(claimed), "{resting} claims {claimed}");
    }
}

#[test]
fn nothing_at_the_chosen_quality_says_so_and_says_what_eases_it() {
    // Releases exist and the profile wants none of them is the one stall an operator
    // can end by choosing differently, so it names both halves: that there is
    // something out there, and that the quality in force is what is refusing it.
    let unmet = Stage::Found.stall().unwrap_or_default();
    assert!(unmet.contains("quality"), "{unmet}");
    assert!(unmet.contains("easing"), "{unmet}");
}

#[test]
fn a_stage_serialises_under_its_label() {
    let json = serde_json::to_string(&Stage::Downloaded).unwrap_or_default();
    assert_eq!(json, r#""downloaded""#);
    let back: Option<Stage> = serde_json::from_str(&json).ok();
    assert_eq!(back, Some(Stage::Downloaded));
}

#[test]
fn the_furthest_stage_is_the_latest_reached() {
    assert_eq!(Stage::furthest(false, &[]), Stage::NotMonitored);
    // Unmonitored floors even if history somehow shows more.
    assert_eq!(
        Stage::furthest(false, &[Stage::Grabbed]),
        Stage::NotMonitored
    );
    assert_eq!(Stage::furthest(true, &[]), Stage::Monitored);
    assert_eq!(
        Stage::furthest(true, &[Stage::Grabbed, Stage::Imported]),
        Stage::Imported
    );
    // Order of the events does not matter — the furthest is a max.
    assert_eq!(
        Stage::furthest(true, &[Stage::Imported, Stage::Grabbed]),
        Stage::Imported
    );
}

#[test]
fn history_events_map_to_the_outcome_they_denote() {
    assert_eq!(Outcome::of_event("grabbed"), Some(Outcome::Grabbed));
    assert_eq!(
        Outcome::of_event("downloadFailed"),
        Some(Outcome::DownloadFailed)
    );
    for imported in [
        "downloadFolderImported",
        "movieFolderImported",
        "seriesFolderImported",
    ] {
        assert_eq!(Outcome::of_event(imported), Some(Outcome::Imported));
    }
    assert_eq!(
        Outcome::of_event("episodeFileDeleted"),
        Some(Outcome::Removed)
    );
    assert_eq!(
        Outcome::of_event("movieFileDeleted"),
        Some(Outcome::Removed)
    );
}

#[test]
fn only_a_forward_outcome_advances_the_stage() {
    // A grab and an import move the item along; a failure or a removal is history to
    // show, not a stage reached.
    assert_eq!(Outcome::Grabbed.stage(), Some(Stage::Grabbed));
    assert_eq!(Outcome::Imported.stage(), Some(Stage::Imported));
    assert_eq!(Outcome::DownloadFailed.stage(), None);
    assert_eq!(Outcome::Removed.stage(), None);
}

#[test]
fn every_outcome_has_a_plain_phrase() {
    for outcome in [
        Outcome::Grabbed,
        Outcome::DownloadFailed,
        Outcome::Imported,
        Outcome::Removed,
    ] {
        let phrase = outcome.phrase();
        assert!(!phrase.is_empty());
        assert!(phrase.chars().all(|c| c.is_ascii_lowercase() || c == ' '));
    }
}

#[test]
fn queue_states_map_to_the_stage_they_are_at() {
    assert_eq!(
        Stage::of_queue_state("downloading"),
        Some(Stage::Downloading)
    );
    assert_eq!(
        Stage::of_queue_state("importPending"),
        Some(Stage::Downloaded)
    );
    assert_eq!(Stage::of_queue_state("importing"), Some(Stage::Importing));
    assert_eq!(Stage::of_queue_state("imported"), Some(Stage::Imported));
    assert_eq!(Stage::of_queue_state("failed"), None);
    assert_eq!(Stage::of_queue_state(""), None);
}

#[test]
fn an_event_that_is_not_notable_maps_to_no_outcome() {
    // A rename, a grab-from-interactive-search test, or an unknown event is not one
    // of the history moments a trace shows.
    assert_eq!(Outcome::of_event("episodeFileRenamed"), None);
    assert_eq!(Outcome::of_event(""), None);
}

/// A part at a given season, number and stage — the shape the aggregation groups.
fn part(season: u32, number: u32, stage: Stage) -> Part {
    Part {
        season,
        number,
        title: format!("S{season:02}E{number:02}"),
        stage,
    }
}

#[test]
fn a_part_on_disk_is_here_whoever_stopped_monitoring_it() {
    // The file is the fact. An episode fetched and then unmonitored is still here,
    // and calling it "nobody asked for it" would be the worse answer.
    assert_eq!(Stage::of_part(false, true), Stage::Imported);
    assert_eq!(Stage::of_part(true, true), Stage::Imported);
}

#[test]
fn an_unmonitored_absent_part_is_one_nobody_asked_for() {
    assert_eq!(Stage::of_part(false, false), Stage::NotMonitored);
}

#[test]
fn a_wanted_part_with_no_file_rests_at_monitored() {
    // Where it starts from: what was tried for it is folded in by the caller, which
    // holds the history and queue this knows nothing about.
    assert_eq!(Stage::of_part(true, false), Stage::Monitored);
}

#[test]
fn coverage_counts_what_was_asked_for_and_reports_the_rest_apart() {
    // A season of three wanted episodes, one here — plus a special nobody asked for,
    // which must not drag the denominator to four and read as a fault.
    let coverage = Coverage::of(vec![
        part(1, 2, Stage::Monitored),
        part(1, 1, Stage::Imported),
        part(1, 3, Stage::Downloading),
        part(0, 1, Stage::NotMonitored),
    ]);
    assert_eq!(coverage.have, 1);
    assert_eq!(coverage.wanted, 3);
    assert_eq!(coverage.unmonitored, 1);
    // Seasons come out in number order, specials first as season zero.
    let numbers: Vec<u32> = coverage.seasons.iter().map(|s| s.season).collect();
    assert_eq!(numbers, vec![0, 1]);
}

#[test]
fn outstanding_parts_are_the_wanted_ones_not_here_yet_in_order() {
    let coverage = Coverage::of(vec![
        part(2, 3, Stage::Monitored),
        part(2, 1, Stage::Imported),
        part(2, 2, Stage::Downloading),
        part(2, 4, Stage::NotMonitored),
    ]);
    let season = coverage.seasons.first().cloned().unwrap_or_default();
    // Sorted by number whatever order the service listed them in, so the reading is
    // stable; the one nobody asked for is not outstanding, and the one already here
    // is not either.
    let outstanding: Vec<u32> = season.outstanding.iter().map(|p| p.number).collect();
    assert_eq!(outstanding, vec![2, 3]);
    // Each carries its own stage, so a download in flight is told apart from a stall.
    let stages: Vec<Stage> = season.outstanding.iter().map(|part| part.stage).collect();
    assert_eq!(stages, vec![Stage::Downloading, Stage::Monitored]);
}

#[test]
fn a_season_is_complete_when_every_wanted_part_is_here() {
    let coverage = Coverage::of(vec![
        part(1, 1, Stage::Imported),
        part(1, 2, Stage::Imported),
    ]);
    assert!(coverage.complete());
    let only = coverage.seasons.first().cloned().unwrap_or_default();
    assert!(only.complete());
    assert_eq!(only.wanted, 2);
}

#[test]
fn a_series_nobody_monitors_is_not_complete() {
    // Nothing wanted is not the same as everything here — with no denominator there
    // is nothing to be complete, and reporting it complete would be a lie of shape.
    let coverage = Coverage::of(vec![part(1, 1, Stage::NotMonitored)]);
    assert!(!coverage.complete());
    let only = coverage.seasons.first().cloned().unwrap_or_default();
    assert!(!only.complete());
    assert_eq!(only.unmonitored, 1);
    // And an item with no parts at all — a film — is not a complete series either.
    assert!(!Coverage::of(Vec::new()).complete());
}

#[test]
fn a_part_reports_whether_it_is_here_and_whether_it_was_asked_for() {
    assert!(part(1, 1, Stage::Imported).here());
    assert!(part(1, 1, Stage::Available).here());
    assert!(!part(1, 1, Stage::Downloading).here());
    assert!(part(1, 1, Stage::NotMonitored).unasked());
    assert!(!part(1, 1, Stage::Monitored).unasked());
}

#[test]
fn confidence_serialises_under_its_label() {
    assert_eq!(
        serde_json::to_string(&Confidence::Uncertain).unwrap_or_default(),
        r#""uncertain""#
    );
}
