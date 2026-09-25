use super::{keep, kept};
use std::collections::BTreeSet;

/// A scratch record path for one test.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name).within("record.json")
}

#[test]
fn what_was_written_is_what_comes_back() {
    let path = scratch("round-trip");
    let mut written = BTreeSet::new();
    written.insert("one".to_owned());
    assert!(keep(Some(path.path()), &written).is_ok());
    assert_eq!(kept::<BTreeSet<String>>(Some(path.path())), written);
}

#[test]
fn a_record_that_is_not_there_reads_as_the_default() {
    // A machine with no history is not a fault; it is a machine with no
    // history.
    assert!(kept::<BTreeSet<String>>(Some(scratch("absent").path())).is_empty());
}

#[test]
fn a_record_that_will_not_parse_reads_as_the_default_rather_than_a_failure() {
    // The safe direction: a settled question is put again, or a fault's age is
    // forgotten. The alternative is refusing to run over a file nobody needs.
    let path = scratch("corrupt");
    assert!(keep(Some(path.path()), &BTreeSet::<String>::new()).is_ok());
    assert!(crate::config::store::write(&path, "not json at all").is_ok());
    assert!(kept::<BTreeSet<String>>(Some(path.path())).is_empty());
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
    // A directory where the record would be written before it is moved into
    // place. Telling the operator something was remembered when it was not is the
    // failure worth avoiding here.
    let path = scratch("blocked");
    assert!(
        std::fs::create_dir_all(path.with_file_name("record.json.writing").join("held")).is_ok()
    );
    let refused = keep(Some(path.path()), &BTreeSet::<String>::new());
    assert!(
        refused.is_err_and(|problem| problem.summary.contains("could not be saved")),
        "the refusal is the write's"
    );
}

/// A record that cannot be read is not written over with the default it read as.
///
/// The value a caller writes back was worked out from that default, so writing it
/// would replace what the file still held. The file is left as it was.
#[test]
fn a_record_that_will_not_parse_is_not_written_over() {
    let path = scratch("damaged");
    assert!(crate::config::store::write(&path, "not json at all").is_ok());

    let refused = keep(Some(path.path()), &BTreeSet::<String>::new());
    assert!(refused.is_err(), "the write was refused");
    assert_eq!(
        std::fs::read_to_string(&*path).ok().as_deref(),
        Some("not json at all"),
        "and the file is as it was"
    );
}

/// A record there and unreadable is not written over either.
#[test]
fn a_record_that_cannot_be_read_is_not_written_over() {
    let path = scratch("unreadable");
    // A directory with something in it where the record is: it cannot be read as a
    // file, and a write refusing it is what is asked.
    assert!(std::fs::create_dir_all(path.join("held")).is_ok());
    let refused = keep(Some(path.path()), &BTreeSet::<String>::new());
    assert!(
        refused.is_err_and(|problem| problem.summary.contains("could not be read")),
        "the refusal is the unreadable record's"
    );
}
