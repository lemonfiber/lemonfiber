//! Which stage an item reached, and where it stalled.

use super::*;

#[test]
fn a_monitored_item_with_a_grab_and_import_reaches_imported() {
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(
            vec![event(Outcome::Imported), event(Outcome::Grabbed)],
            Vec::new(),
            None,
        ),
    );
    assert!(report.matched);
    assert_eq!(report.furthest, Stage::Imported);
    // Monitored + the two events, oldest-first.
    assert_eq!(report.stages.len(), 3);
    assert_eq!(
        report.stages.first().map(|s| s.stage),
        Some(Stage::Monitored)
    );
    assert_eq!(report.stages.last().map(|s| s.stage), Some(Stage::Imported));
    // Imported with the media server unread is not a stall: it may already be
    // scanned, and the *arr cannot tell — so nothing is claimed.
    assert!(report.stall.is_none());
}

#[test]
fn a_monitored_item_with_no_history_says_which_question_is_open() {
    // Nothing grabbed, and no search made. Two causes look identical from here —
    // the indexers carry nothing, and they carry only what the quality in force
    // rejects — so the reason names neither and says what would tell them apart.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(Vec::new(), Vec::new(), None),
    );
    assert_eq!(report.furthest, Stage::Monitored);
    let reason = report.stall.unwrap_or_default();
    assert!(reason.contains("no search was run"), "{reason}");
    assert!(reason.contains("quality in force"), "{reason}");
    assert!(reason.contains("ask for a search"), "{reason}");
}

#[test]
fn a_repeated_attempt_shows_in_the_history_the_furthest_stage_flattens() {
    // Grabbed, the download failed, grabbed again: the furthest stage is a single
    // "grabbed" — a failure advances nothing — but the history keeps all three, oldest
    // first, so the repeated attempt is the pattern it is rather than one flat stage.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(
            vec![
                event(Outcome::Grabbed),
                event(Outcome::DownloadFailed),
                event(Outcome::Grabbed),
            ],
            Vec::new(),
            None,
        ),
    );
    assert_eq!(report.furthest, Stage::Grabbed);
    let outcomes: Vec<Outcome> = report.history.iter().map(|moment| moment.outcome).collect();
    assert_eq!(
        outcomes,
        vec![Outcome::Grabbed, Outcome::DownloadFailed, Outcome::Grabbed]
    );
}

#[test]
fn a_removal_after_an_import_shows_in_the_history() {
    // Imported then the file was removed: the history shows the full story including
    // the removal, though "removed" is not a stage the pipeline advances to, so the
    // furthest reached is still imported.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(
            vec![event(Outcome::Removed), event(Outcome::Imported)],
            Vec::new(),
            None,
        ),
    );
    assert_eq!(report.furthest, Stage::Imported);
    let outcomes: Vec<Outcome> = report.history.iter().map(|moment| moment.outcome).collect();
    assert_eq!(outcomes, vec![Outcome::Imported, Outcome::Removed]);
}

#[test]
fn an_unmonitored_item_is_reported_as_nobody_asked() {
    let report = assemble(
        "Sonarr",
        "The Expanse",
        false,
        frags(Vec::new(), Vec::new(), None),
    );
    assert!(!report.matched);
    assert_eq!(report.furthest, Stage::NotMonitored);
    assert!(report.stall.is_some());
}

#[test]
fn a_grab_that_never_reached_the_queue_is_stuck_at_grabbed() {
    // Grabbed in history, nothing in the queue, never imported: the download client
    // never took it — now provable, so it stalls at grabbed.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(vec![event(Outcome::Grabbed)], Vec::new(), None),
    );
    assert_eq!(report.furthest, Stage::Grabbed);
    assert!(report
        .stall
        .as_deref()
        .is_some_and(|reason| reason.contains("download client never took it")));
}

#[test]
fn a_queued_download_carries_the_trace_to_downloading() {
    // Grabbed in history and downloading in the queue: the queue advances the trace,
    // and downloading is in progress — not a stall.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(
            vec![event(Outcome::Grabbed)],
            queued(Stage::Downloading, false),
            None,
        ),
    );
    assert_eq!(report.furthest, Stage::Downloading);
    assert_eq!(
        report.stages.last().map(|s| s.stage),
        Some(Stage::Downloading)
    );
    assert!(report.stall.is_none());
}

#[test]
fn a_stuck_queue_item_stalls_whatever_stage_it_reached() {
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(
            vec![event(Outcome::Grabbed)],
            queued(Stage::Downloading, true),
            None,
        ),
    );
    assert!(report
        .stall
        .as_deref()
        .is_some_and(|reason| reason.contains("not progressing")));
}

#[test]
fn a_monitored_item_present_in_the_library_reaches_available() {
    // The media server confirms it: the trace runs to available, ends on the library
    // stage, and — a cross-service title match — is marked uncertain, not claimed.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(
            vec![event(Outcome::Imported)],
            Vec::new(),
            Some(Presence::Present),
        ),
    );
    assert_eq!(report.furthest, Stage::Available);
    assert_eq!(
        report.stages.last().map(|s| s.stage),
        Some(Stage::Available)
    );
    assert_eq!(report.confidence, crate::trace::Confidence::Uncertain);
    assert!(report.stall.is_none());
}

#[test]
fn imported_but_absent_from_the_library_stalls_as_not_scanned() {
    // Imported to disk, and the media server confirms it is not there: now provably
    // still waiting for the library to be scanned — a stall only a confirmed absence
    // earns, and the confidence stays certain because no fuzzy match was made.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(
            vec![event(Outcome::Imported)],
            Vec::new(),
            Some(Presence::Absent),
        ),
    );
    assert_eq!(report.furthest, Stage::Imported);
    assert_eq!(report.confidence, crate::trace::Confidence::Certain);
    assert!(report
        .stall
        .as_deref()
        .is_some_and(|reason| reason.contains("has not been scanned")));
}

#[test]
fn a_library_match_for_an_unmonitored_item_is_a_disagreement_not_availability() {
    // Nobody asked for it, yet the library has something by that title: the services
    // disagree. "Not monitored" is still how far it got — the stray match never
    // promotes it to available or marks the trace uncertain — but the contradiction is
    // surfaced as a finding rather than reconciled away.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        false,
        frags(Vec::new(), Vec::new(), Some(Presence::Present)),
    );
    assert_eq!(report.furthest, Stage::NotMonitored);
    assert_eq!(report.confidence, crate::trace::Confidence::Certain);
    assert!(report
        .stall
        .as_deref()
        .is_some_and(|reason| reason.contains("nobody has asked")));
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("no service is monitoring it")));
}

#[test]
fn agreeing_services_raise_no_disagreement() {
    // Monitored and present: the services agree, so there is no finding — a disagreement
    // is only raised where two views genuinely contradict.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        frags(
            vec![event(Outcome::Imported)],
            Vec::new(),
            Some(Presence::Present),
        ),
    );
    assert_eq!(report.furthest, Stage::Available);
    assert!(report.findings.is_empty());
}

#[test]
fn an_unreadable_history_is_not_read_as_never_found() {
    // The history could not be read: an empty result must not be taken as "indexers
    // returned nothing" — the gap is reported as unavailable, not inferred as nothing.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        Fragments {
            events: Vec::new(),
            queue: Vec::new(),
            parts: Vec::new(),
            library: None,
            reads: Reads {
                history: false,
                ..Reads::ALL
            },
        },
    );
    assert_eq!(report.furthest, Stage::Monitored);
    assert!(report.stall.is_none());
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("history could not be read")));
}

#[test]
fn an_unreadable_queue_does_not_prove_a_grab_stuck() {
    // Grabbed in history, but the queue could not be read: "the client never took it"
    // is a claim about an empty queue, so it is not made — the gap is reported instead.
    let report = assemble(
        "Sonarr",
        "The Expanse",
        true,
        Fragments {
            events: vec![event(Outcome::Grabbed)],
            queue: Vec::new(),
            parts: Vec::new(),
            library: None,
            reads: Reads {
                queue: false,
                ..Reads::ALL
            },
        },
    );
    assert_eq!(report.furthest, Stage::Grabbed);
    assert!(report.stall.is_none());
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("queue could not be read")));
}
