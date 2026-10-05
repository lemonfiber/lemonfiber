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
///
/// The second and third are noted while the first is being written, so whoever
/// writes next takes both — and the one left with nothing to write leaves the file
/// alone rather than writing an empty line.
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
