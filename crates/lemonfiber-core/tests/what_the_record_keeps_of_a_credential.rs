//! What the change journal leaves on disk where a setting held a credential.
//!
//! Beside the crate rather than inside it because this is about the file, not about the
//! function that wrote it: the assertion is made by reading `journal.jsonl` back as bytes
//! and looking for a password in it, which is what somebody who walked off with the file
//! would do. A test that inspected the `Change` the writer was handed would pass over the
//! one thing that matters here.
//!
//! The record has to keep these values — putting a change back writes what the setting
//! held before it — so the answer is neither to leave them out nor to leave them in
//! clear. What is asserted is all three halves of that: the file carries nothing a
//! credential can be read out of, a read of the same file still hands the value back so a
//! reversal has something to put back, and a setting that is not a credential is still
//! recorded as itself so the record stays browsable.

use std::path::{Path, PathBuf};

use lemonfiber_core::app::recover::{journal_at, journalled, undo, NOT_OPENED};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::journal::{Change, Journal, Kind};
use lemonfiber_fixtures::ports::Chance;

/// Where this test's records live, in a scratch directory of its own.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-sealed-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn paths(root: &Path) -> Paths {
    Paths::rooted(&root.join("config"), &root.join("data"))
}

/// The randomness a real machine supplies.
fn a_machine() -> Chance {
    Chance::cycling()
}

/// One setting written by an operation.
fn set(key: &str, previous: Option<&str>, current: &str) -> Change {
    Change {
        at: "2000".to_owned(),
        operation: "reconfigure".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: current.to_owned(),
        },
    }
}

/// The record as it stands in memory, which is what a reversal reads it from.
///
/// Serialised whole rather than picked apart, so an assertion about what came back is
/// about every field of every entry and not about the one the test remembered to name.
fn as_read(journal: &Journal) -> String {
    serde_json::to_string(journal.changes()).unwrap_or_default()
}

/// A credential setting, and the two values one has held.
const KEY: &str = "INDEXER_APIKEY";

/// Built rather than written, so the literal is not a password sitting in a source file
/// for a secret scanner to find.
fn was() -> String {
    format!("old-{}", "s3cret")
}

fn now() -> String {
    format!("new-{}", "s3cret")
}

/// The defect this exists to catch: the journal is the one file here that keeps a
/// credential nothing reads, and it kept it as itself.
///
/// Both values, because both are recorded and either one is the whole leak. The value a
/// setting was changed *from* is the worse of the two: it is the credential the operator
/// has since rotated away from, and a file that goes on holding it has undone the point
/// of rotating it.
#[test]
fn no_credential_is_left_in_the_file_the_record_is_kept_in() {
    let root = scratch("in-the-clear");
    let journal = paths(&root).journal();

    journalled(
        &journal,
        &[set(KEY, Some(&was()), &now()), set(KEY, None, &now())],
        &a_machine(),
    );

    let written = std::fs::read_to_string(&journal).unwrap_or_default();
    assert!(!written.is_empty(), "the record was written");
    assert!(
        !written.contains(&was()) && !written.contains(&now()),
        "the file carries neither value: {written}"
    );
    assert!(
        written.contains(KEY),
        "and still names the setting, because which one changed is the useful half"
    );
}

/// The other half of the same rule: what is not a credential is recorded as itself.
///
/// A journal nobody can read is a journal nobody consults, and this is what an operator
/// reads to answer "why is this different from last week?". Sealing everything would be
/// the cheap way to pass the test above and would cost the whole feature.
#[test]
fn a_setting_that_is_not_a_credential_is_recorded_as_what_it_is() {
    let root = scratch("in-the-open");
    let journal = paths(&root).journal();

    journalled(
        &journal,
        &[set("TZ", Some("UTC"), "Europe/Amsterdam")],
        &a_machine(),
    );

    let written = std::fs::read_to_string(&journal).unwrap_or_default();
    assert!(written.contains("Europe/Amsterdam"), "{written}");
    assert!(written.contains("UTC"), "{written}");
}

/// Sealed, not left out — which is the difference between a reversal and an apology.
///
/// The value is read back through the same path a rollback reads it through, so what is
/// asserted is that the record still holds what putting the change back would write.
#[test]
fn the_record_still_hands_back_what_a_reversal_would_put_there() {
    let root = scratch("read-back");
    let journal = paths(&root).journal();

    journalled(&journal, &[set(KEY, Some(&was()), &now())], &a_machine());

    let held = as_read(&journal_at(&journal));

    assert!(
        held.contains(&was()) && held.contains(&now()),
        "both values come back to the one caller entitled to them: {held}"
    );
}

/// A journal an older version left in clear still reads, and stops being one.
///
/// The upgrade path, and it is not an optional nicety: a machine that has been running
/// since before this has a file full of credentials, and a fix that only applied to
/// records written afterwards would leave every one of them exactly where it was.
#[test]
fn a_record_written_in_clear_reads_and_is_sealed_the_next_time_anything_is_added() {
    let root = scratch("upgrading");
    let journal = paths(&root).journal();
    let older = serde_json::to_string(&set(KEY, None, &was())).unwrap_or_default();
    if let Some(dir) = journal.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    assert!(std::fs::write(&journal, format!("{older}\n")).is_ok());

    // It reads as it always did, before anything has been written over it.
    let before = as_read(&journal_at(&journal));
    assert!(
        before.contains(&was()),
        "the old record still reads: {before}"
    );

    journalled(
        &journal,
        &[set("TZ", None, "Europe/Amsterdam")],
        &a_machine(),
    );

    let written = std::fs::read_to_string(&journal).unwrap_or_default();
    assert!(
        !written.contains(&was()),
        "the credential the older version wrote is gone from the file: {written}"
    );
    let after = as_read(&journal_at(&journal));
    assert!(
        after.contains(&was()),
        "and is still there to put back: {after}"
    );
}

/// The key is kept beside the record, and is as private as the record is.
///
/// Only on unix: the guarantee is a file mode, which is the platform's own notion.
#[cfg(unix)]
#[test]
fn the_key_is_written_owner_only_where_the_platform_has_the_notion() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = scratch("private");
    let layout = paths(&root);
    journalled(&layout.journal(), &[set(KEY, None, &now())], &a_machine());

    let mode =
        std::fs::metadata(layout.journal_key()).map(|kept| kept.permissions().mode() & 0o777);
    assert_eq!(mode.ok(), Some(0o600), "readable only by its owner");
}

/// Without the key, a reversal refuses rather than writing the sealed text into the
/// environment file.
///
/// The state a machine restored from a backup that took the records and left the key is
/// in. Putting the sealed text back would report the setting restored and leave the
/// operator authenticating with a line of hexadecimal — worse than not reversing, because
/// it reads as having worked.
#[test]
fn a_record_whose_key_is_gone_is_refused_rather_than_put_back_as_it_reads() {
    let root = scratch("no-key");
    let layout = paths(&root);
    journalled(
        &layout.journal(),
        &[set(KEY, Some(&was()), &now())],
        &a_machine(),
    );
    assert!(std::fs::remove_file(layout.journal_key()).is_ok());

    let undos = journal_at(&layout.journal()).rewind();
    let refused = undo(&undos, &layout.env_file(), Vec::new());

    let problem = refused.err();
    assert_eq!(
        problem.as_ref().map(|refusal| refusal.code),
        Some(NOT_OPENED),
        "refused for the reason it was, not for the one a missing setting would give"
    );
    assert_eq!(
        problem.and_then(|refusal| refusal.detail.clone()),
        Some(KEY.to_owned()),
        "the setting is named, so the operator knows which credential to set again"
    );
    let file = std::fs::read_to_string(layout.env_file()).unwrap_or_default();
    assert!(
        !file.contains("sealed:"),
        "and nothing sealed was written into the settings: {file}"
    );
}
