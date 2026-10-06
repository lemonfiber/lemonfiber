use super::{Ledger, KEPT, TRIM_AT};

/// A directory of its own for one test's record.
///
/// Writing the record makes its parent owner-only, so a test pointing at the shared
/// temporary directory would take everyone else's out from under them.
fn record(name: &str) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("outbound.log")
}

/// Lines noted together all arrive, each whole and in the order noted.
#[tokio::test]
async fn lines_noted_together_all_arrive_whole() {
    let at = record("together");
    let ledger = Ledger::at(at.clone());

    tokio::join!(
        ledger.note("1 first".to_owned()),
        ledger.note("2 second".to_owned()),
        ledger.note("3 third".to_owned()),
    );

    let written = std::fs::read_to_string(&at).unwrap_or_default();
    assert_eq!(written, "1 first\n2 second\n3 third\n");
    let _ = at.parent().map(std::fs::remove_dir_all);
}

/// A line already written by whoever wrote before is not written again, and the one
/// left with nothing to write leaves the file alone.
///
/// Both lines are queued while a write is held in progress, so whoever is let in first
/// takes both and the other finds nothing waiting. Held here rather than left to how
/// quickly a write finishes, which decides on its own whether anybody is left waiting.
#[tokio::test]
async fn whoever_writes_next_takes_every_line_queued_and_the_other_writes_nothing() {
    let at = record("taken");
    let ledger = std::sync::Arc::new(Ledger::at(at.clone()));
    let held = ledger.writing.lock().await;

    let first = tokio::spawn({
        let ledger = ledger.clone();
        async move { ledger.note("1 first".to_owned()).await }
    });
    let second = tokio::spawn({
        let ledger = ledger.clone();
        async move { ledger.note("2 second".to_owned()).await }
    });
    while ledger.waiting.lock().map_or(0, |waiting| waiting.len()) < 2 {
        tokio::task::yield_now().await;
    }
    drop(held);
    let (first, second) = tokio::join!(first, second);

    assert!(first.is_ok() && second.is_ok());
    let written = std::fs::read_to_string(&at).unwrap_or_default();
    assert_eq!(written, "1 first\n2 second\n");
    assert!(ledger
        .waiting
        .lock()
        .is_ok_and(|waiting| waiting.is_empty()));
    let _ = at.parent().map(std::fs::remove_dir_all);
}

/// The record is bounded, and it is the oldest that goes.
#[tokio::test]
async fn the_record_keeps_what_is_recent_rather_than_growing_for_ever() {
    let at = record("bounded");
    let ledger = Ledger::at(at.clone());
    let filler = "x".repeat(200);
    let mut noted = 0_usize;
    while std::fs::metadata(&at).map_or(0, |meta| meta.len()) <= TRIM_AT / 2 || noted < KEPT * 3 {
        ledger.note(format!("{noted} {filler}")).await;
        noted += 1;
    }
    ledger.note("the newest".to_owned()).await;

    let written = std::fs::read_to_string(&at).unwrap_or_default();
    assert!(
        std::fs::metadata(&at).map_or(u64::MAX, |meta| meta.len()) <= TRIM_AT,
        "the record grew past its bound"
    );
    assert!(
        written.lines().count() >= KEPT,
        "fewer than the kept lines remain"
    );
    assert_eq!(written.lines().last(), Some("the newest"));
    assert!(!written.starts_with("0 "), "the oldest went");
    let _ = at.parent().map(std::fs::remove_dir_all);
}

/// The record is readable by its owner alone.
#[cfg(unix)]
#[tokio::test]
async fn the_record_is_its_owners_alone() {
    use std::os::unix::fs::PermissionsExt as _;
    let at = record("private");
    Ledger::at(at.clone()).note("1 one".to_owned()).await;
    let mode = std::fs::metadata(&at).map_or(0, |meta| meta.permissions().mode() & 0o777);
    assert_eq!(mode, 0o600);
    let _ = at.parent().map(std::fs::remove_dir_all);
}

/// A record that cannot be written is not a failure anybody hears about.
#[tokio::test]
async fn a_record_that_cannot_be_written_is_passed_over() {
    let at = record("blocked");
    let _ = at.parent().map(std::fs::create_dir_all);
    let _ = std::fs::create_dir_all(&at);
    Ledger::at(at.clone()).note("1 one".to_owned()).await;
    assert!(at.is_dir(), "nothing was written over what was there");
    let _ = at.parent().map(std::fs::remove_dir_all);
}

/// A record whose directory cannot be made is let go: noting a line returns, and
/// nothing is written in place of what stood where the directory would go.
#[tokio::test]
async fn a_record_with_nowhere_to_go_is_let_go() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("nowhere-to-go");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let blocking = dir.join("a-file");
    let _ = std::fs::write(&blocking, "not a directory");

    Ledger::at(blocking.join("outbound.log"))
        .note("1 lost".to_owned())
        .await;

    assert_eq!(
        std::fs::read_to_string(&blocking).ok().as_deref(),
        Some("not a directory")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
