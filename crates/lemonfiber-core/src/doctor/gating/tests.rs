use std::path::{Path, PathBuf};

use lemonfiber_sidecar::gate::{Kept, Outcome, Record};

use super::{Gate, GateRecordCheck, LOST, REFUSED};
use crate::config::store;
use crate::doctor::{Category, Check, Finding, Verdict};

/// A record holding `entries`, each a route, method, path and outcome, at one-minute
/// steps from the epoch, with all but the last `kept` fallen off.
fn record(entries: &[(&str, &str, &str, Outcome)], kept: usize) -> Record {
    let mut record = Record::default();
    for (step, (route, method, path, outcome)) in entries.iter().enumerate() {
        record = record.with(
            u64::try_from(step).unwrap_or_default() * 60,
            route,
            method,
            path,
            *outcome,
            Kept::standard(),
        );
    }
    let over = record.entries.len().saturating_sub(kept);
    record.entries.drain(..over);
    record
}

/// A scratch directory holding `record` where there is one, and the place last read
/// at `read` where there is one.
fn scene(name: &str, record: Option<&str>, read: Option<&str>) -> PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    if let Some(record) = record {
        let _ = store::write(&at.join("record.json"), record);
    }
    if let Some(read) = read {
        let _ = store::write(&at.join("gate-read.json"), read);
    }
    at
}

/// The check over the gate under `at`, keeping its place there where `keeping`.
fn check(at: &Path, keeping: bool) -> GateRecordCheck {
    GateRecordCheck::new(
        files(),
        Some(Gate {
            record: at.join("record.json"),
            read: keeping.then(|| at.join("gate-read.json")),
        }),
    )
}

/// The filesystem the checks read through.
fn files() -> std::sync::Arc<dyn crate::ports::filesystem::FileSystem> {
    crate::test_support::a_context()
        .build()
        .seams
        .filesystem
        .clone()
}

async fn verdicts(check: &GateRecordCheck) -> Vec<Verdict> {
    check
        .run()
        .await
        .into_iter()
        .map(|finding: Finding| finding.verdict)
        .collect()
}

fn note(verdict: &Verdict) -> Option<&str> {
    match verdict {
        Verdict::Pass { note } => note.as_deref(),
        _ => None,
    }
}

fn code(verdict: &Verdict) -> Option<crate::error::Code> {
    match verdict {
        Verdict::Warn(problem) => Some(problem.code),
        _ => None,
    }
}

const REMOVAL: (&str, &str, &str, Outcome) = (
    "radarr",
    "DELETE",
    "/radarr/api/v3/movie/7",
    Outcome::Removed,
);
const REFUSAL: (&str, &str, &str, Outcome) =
    ("sonarr", "POST", "/sonarr/api/v3/command", Outcome::Refused);

#[test]
fn the_record_is_a_question_about_the_services() {
    assert_eq!(
        GateRecordCheck::new(files(), None).category(),
        Category::Services
    );
}

#[tokio::test]
async fn without_a_gate_or_a_record_there_is_nothing_to_report() {
    let skipped = verdicts(&GateRecordCheck::new(files(), None)).await;
    assert!(
        matches!(skipped.as_slice(), [Verdict::Skipped { reason }] if reason.contains("no request gate")),
        "{skipped:?}"
    );

    let at = scene("gate-record-none", None, None);
    let none = verdicts(&check(&at, true)).await;
    assert_eq!(
        none.first().and_then(note),
        Some("the request gate has refused nothing and removed nothing")
    );
}

#[tokio::test]
async fn an_unreadable_record_is_unverified() {
    let at = scene("gate-record-unreadable", Some("not a record"), None);

    let found = verdicts(&check(&at, true)).await;

    assert!(
        matches!(found.as_slice(), [Verdict::Unverified { reason, .. }] if reason.starts_with("the request gate's record could not be read: ")),
        "{found:?}"
    );
}

#[tokio::test]
async fn refusals_warn_removals_are_told_and_each_is_reported_once() {
    let written = record(&[REMOVAL, REFUSAL, REFUSAL], 1000).written();
    let at = scene("gate-record-new", Some(&written), None);
    let check = check(&at, true);

    let first = verdicts(&check).await;

    assert_eq!(first.first().and_then(code), Some(REFUSED));
    let refused = match first.first() {
        Some(Verdict::Warn(problem)) => problem.summary.clone(),
        other => format!("{other:?}"),
    };
    assert_eq!(
        refused,
        "the request gate refused 2 calls since the last check: 1970-01-01T00:01:00Z POST \
         /sonarr/api/v3/command; 1970-01-01T00:02:00Z POST /sonarr/api/v3/command"
    );
    assert_eq!(
        first.get(1).and_then(note),
        Some(
            "removed through the request service since the last check: 1970-01-01T00:00:00Z \
             DELETE /radarr/api/v3/movie/7"
        )
    );
    assert_eq!(
        std::fs::read_to_string(at.join("gate-read.json"))
            .ok()
            .as_deref(),
        Some("3\n")
    );

    let again = verdicts(&check).await;
    assert_eq!(
        again.first().and_then(note),
        Some("nothing refused or removed since the last check")
    );
}

#[tokio::test]
async fn one_refusal_is_one_call() {
    let written = record(&[REFUSAL], 1000).written();
    let at = scene("gate-record-one", Some(&written), None);

    let found = verdicts(&check(&at, false)).await;

    let said = match found.first() {
        Some(Verdict::Warn(problem)) => problem.summary.clone(),
        other => format!("{other:?}"),
    };
    assert!(
        said.starts_with("the request gate refused 1 call since"),
        "{said}"
    );
    assert!(!at.join("gate-read.json").exists());
}

#[tokio::test]
async fn entries_that_fell_off_unread_are_counted() {
    // Fifteen recorded and the first twelve fallen off: read from the start, twelve
    // were never seen.
    let fifteen: Vec<_> = std::iter::repeat_n(REMOVAL, 15).collect();
    let written = record(&fifteen, 3).written();
    let at = scene("gate-record-lost", Some(&written), Some("0\n"));

    let found = verdicts(&check(&at, true)).await;

    let lost = found
        .iter()
        .find_map(|verdict| match verdict {
            Verdict::Warn(problem) if problem.code == LOST => Some(problem.summary.clone()),
            _ => None,
        })
        .unwrap_or_default();
    assert_eq!(
        lost,
        "12 entries were dropped before they were read, because more than 1000 arrived \
         between checks"
    );
}
