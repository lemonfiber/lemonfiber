use super::Acknowledged;

/// Exercised at run time as well as in the `static` it exists for, because a
/// `const fn` used only in a `static` is evaluated by the compiler and leaves
/// nothing for a coverage run to see.
#[test]
fn nothing_acknowledged_is_a_value_in_its_own_right() {
    let none = Acknowledged::none();

    assert!(none.is_empty());
    assert!(!none.holds("indexer"));
    assert_eq!(none, Acknowledged::default());
}

/// The whole point: a word gone and found out about is not explained again.
#[test]
fn a_word_that_was_acknowledged_is_held() {
    let mut held = Acknowledged::default();

    assert!(held.is_empty());
    assert!(held.take("indexer"), "recording it changed something");
    assert!(held.holds("indexer"));
    assert_eq!(held.len(), 1);
}

/// A word at the start of a sentence is the same word, and `explain` matches
/// without regard to case, so this must too or the two would disagree.
#[test]
fn a_word_is_held_whatever_case_it_was_written_in() {
    let mut held = Acknowledged::default();
    held.take("Indexer");

    assert!(held.holds("indexer"));
    assert!(held.holds("INDEXER"));
}

/// What decides whether the file is written. Rewriting an identical file on
/// every run is churn that shows up in a backup and explains nothing.
#[test]
fn acknowledging_the_same_word_twice_changes_nothing() {
    let mut held = Acknowledged::default();
    held.take("indexer");

    assert!(!held.take("indexer"), "the second time changed nothing");
    assert_eq!(held.len(), 1);
}

#[test]
fn what_was_written_reads_back_the_same() {
    let mut held = Acknowledged::default();
    held.take("hardlink");
    held.take("indexer");

    let stored = held.to_json().unwrap_or_default();
    let read = Acknowledged::parse(&stored);

    assert_eq!(read, held);
    assert!(
        stored.find("hardlink") < stored.find("indexer"),
        "sorted, so the file reads the same twice: {stored}"
    );
}

/// A first run and a damaged record are the same answer, deliberately: both
/// mean this operator should be told what the words are.
#[test]
fn a_record_that_is_not_there_is_an_empty_one() {
    // Stamped with the process, the way this repo's other temp fixtures are:
    // tests run in parallel and two of them sharing a path is a flake.
    let nowhere = lemonfiber_fixtures::scratch::Scratch::named("known-absent-.json");
    let _ = std::fs::remove_file(&nowhere);

    assert!(super::at(&nowhere).is_empty());
}

#[test]
fn what_was_stored_is_read_back_from_where_it_was_put() {
    let path = lemonfiber_fixtures::scratch::Scratch::named("known-.json");
    let mut held = Acknowledged::default();
    held.take("indexer");
    let _ = std::fs::write(&path, held.to_json().unwrap_or_default());

    let read = super::at(&path);
    let _ = std::fs::remove_file(&path);

    assert!(read.holds("indexer"), "{read:?}");
}

/// The cost of getting this wrong is explaining a word somebody already knew.
/// Refusing to run over it would be the far worse answer.
#[test]
fn a_record_that_cannot_be_read_is_an_empty_one() {
    assert_eq!(
        Acknowledged::parse("not json at all"),
        Acknowledged::default()
    );
    assert!(Acknowledged::parse("").is_empty());
}
