use super::{keep, kept};
use std::collections::BTreeSet;

/// A scratch record path for one test.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-record-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("record.json")
}

#[test]
fn what_was_written_is_what_comes_back() {
    let path = scratch("round-trip");
    let mut written = BTreeSet::new();
    written.insert("one".to_owned());
    assert!(keep(Some(path.as_path()), &written).is_ok());
    assert_eq!(kept::<BTreeSet<String>>(Some(path.as_path())), written);
}

#[test]
fn a_record_that_is_not_there_reads_as_the_default() {
    // A machine with no history is not a fault; it is a machine with no
    // history.
    assert!(kept::<BTreeSet<String>>(Some(scratch("absent").as_path())).is_empty());
}

#[test]
fn a_record_that_will_not_parse_reads_as_the_default_rather_than_a_failure() {
    // The safe direction: a settled question is put again, or a fault's age is
    // forgotten. The alternative is refusing to run over a file nobody needs.
    let path = scratch("corrupt");
    assert!(keep(Some(path.as_path()), &BTreeSet::<String>::new()).is_ok());
    assert!(crate::config::store::write(&path, "not json at all").is_ok());
    assert!(kept::<BTreeSet<String>>(Some(path.as_path())).is_empty());
}

#[test]
fn nowhere_to_keep_it_reads_as_the_default_and_refuses_to_write() {
    // Reading and writing part company here, and deliberately: nothing to read
    // is ordinary, and nowhere to write is something the operator is owed.
    assert!(kept::<BTreeSet<String>>(None).is_empty());
    assert!(keep(None, &BTreeSet::<String>::new()).is_err());
}

#[test]
fn a_record_that_cannot_be_written_is_reported_rather_than_swallowed() {
    // A directory where the file must go. Telling the operator something was
    // remembered when it was not is the failure worth avoiding here.
    let path = scratch("blocked");
    assert!(std::fs::create_dir_all(&path).is_ok());
    assert!(keep(Some(path.as_path()), &BTreeSet::<String>::new()).is_err());
}
