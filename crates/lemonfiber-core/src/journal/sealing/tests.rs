use lemonfiber_fixtures::ports::Chance;

use super::{beside, is_sealed, store, unhex, Change, Kind, Seal, MARKER};

/// A scratch directory unique to this process and case, cleared first.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name).within("journal.jsonl")
}

/// The randomness a real machine supplies.
fn a_machine() -> Chance {
    Chance::cycling()
}

/// A machine that answers with exactly these bytes, however many were asked for.
fn answering(bytes: Option<Vec<u8>>) -> Chance {
    Chance::exactly(bytes)
}

/// One credential setting, changed from something to something else.
fn credential(previous: Option<&str>) -> Change {
    Change {
        at: "t".to_owned(),
        operation: "reconfigure".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: "INDEXER_APIKEY".to_owned(),
            previous: previous.map(str::to_owned),
            current: "what-it-holds-now".to_owned(),
        },
    }
}

/// The change as it reaches the file, which is where an assertion about what is on
/// disk has to be made.
fn as_written(change: &Change) -> String {
    serde_json::to_string(change).unwrap_or_default()
}

/// What a credential setting that replaced nothing is written as where nothing could
/// be sealed: the marker, and nothing after it, in place of the value.
fn unsealable() -> Change {
    Change {
        kind: Kind::Set {
            key: "INDEXER_APIKEY".to_owned(),
            previous: None,
            current: MARKER.to_owned(),
        },
        ..credential(None)
    }
}

#[test]
fn a_credential_is_sealed_on_the_way_out_and_opened_on_the_way_back() {
    let journal = scratch("round-trip");
    let seal = Seal::minted(&journal, &a_machine());
    let change = credential(Some("what-it-held-before"));

    let written = as_written(&seal.sealing(&change, &a_machine()));
    assert!(
        !written.contains("what-it-held-before") && !written.contains("what-it-holds-now"),
        "neither value reaches the file: {written}"
    );
    assert_eq!(written.matches(MARKER).count(), 2, "both halves: {written}");

    assert_eq!(
        seal.opening(&seal.sealing(&change, &a_machine())),
        change,
        "and both come back"
    );
    // A journal an older version wrote holds the value itself, which is not sealed
    // and so is handed back as it stands rather than opened.
    assert_eq!(
        seal.opening(&change),
        change,
        "and a clear one is left alone"
    );
}

/// The key is made once and kept. A second run that minted a fresh one would hold a
/// key that opens nothing already written, which is losing the record without
/// anything saying so.
#[test]
fn the_key_is_made_once_and_read_back_afterwards() {
    let journal = scratch("kept");
    let sealed = Seal::minted(&journal, &a_machine()).sealing(&credential(None), &a_machine());

    let later = Seal::minted(&journal, &a_machine());
    assert_eq!(later.opening(&sealed), credential(None));
    assert_eq!(Seal::kept(&journal).opening(&sealed), credential(None));
}

/// A machine whose randomness will not answer writes no key, and so seals nothing —
/// which must not become "writes the credential as itself".
#[test]
fn a_machine_that_will_not_draw_marks_the_value_rather_than_writing_it() {
    let journal = scratch("no-randomness");
    let seal = Seal::minted(&journal, &answering(None));

    let written = seal.sealing(&credential(None), &answering(None));

    assert_eq!(written, unsealable());
    assert!(!journal.with_file_name(super::KEY_FILE).exists());
}

/// A machine that made a key and then stopped answering seals nothing under it.
///
/// Apart from the machine that would not draw at all, because the key is there and
/// the failure is later: what a value is sealed under is a nonce of its own, and one
/// that cannot be drawn is not something to do without.
#[test]
fn a_machine_that_stops_drawing_after_the_key_seals_nothing_under_it() {
    let journal = scratch("no-nonce");
    let seal = Seal::minted(&journal, &a_machine());

    let written = seal.sealing(&credential(None), &answering(None));

    assert_eq!(written, unsealable());
}

/// Bytes that are not a key are not a key. The port answers with whatever it
/// answers with, and a seal built from a short answer would be a weaker seal
/// invisible in everything it produced.
#[test]
fn randomness_of_the_wrong_length_makes_no_key() {
    let journal = scratch("short-key");
    let seal = Seal::minted(&journal, &answering(Some(vec![9; 8])));

    assert_eq!(seal.sealing(&credential(None), &a_machine()), unsealable());
}

/// A key with nowhere to be written is a key nothing may be sealed under: the
/// alternative is a run that seals against a key the next run cannot find.
#[test]
fn a_key_that_cannot_be_written_down_seals_nothing() {
    let blocker_dir = scratch("unwritable");
    let blocker = blocker_dir.with_file_name("blocker");
    // Written through the store, which makes the directory it needs — so there is no
    // branch here on a parent that is always there.
    assert!(store::write(&blocker, "a file where a directory would go").is_ok());
    let journal = blocker.join("journal.jsonl");

    let seal = Seal::minted(&journal, &a_machine());

    assert_eq!(seal.sealing(&credential(None), &a_machine()), unsealable());
}

/// A nonce of the wrong length seals nothing, for the reason a short key makes none.
#[test]
fn a_nonce_of_the_wrong_length_seals_nothing() {
    let journal = scratch("short-nonce");
    // Thirty-two bytes: enough for the key, and not what a nonce is.
    let machine = answering(Some(vec![3; 32]));
    let seal = Seal::minted(&journal, &machine);

    assert_eq!(seal.sealing(&credential(None), &machine), unsealable());
}

/// A key that is there and will not read is left where it is.
///
/// Writing a fresh one over it would orphan every value already sealed under it, and
/// silently: nothing would fail, and every earlier record would stop opening. The
/// file being briefly unreadable is the likelier explanation, and it is the one that
/// can still be recovered from.
#[test]
fn a_key_that_will_not_read_is_not_written_over() {
    let journal = scratch("unreadable-key");
    let sealed = Seal::minted(&journal, &a_machine()).sealing(&credential(None), &a_machine());
    let kept = beside(&journal);
    let original = std::fs::read_to_string(&kept).unwrap_or_default();
    assert!(std::fs::write(&kept, "not a key at all").is_ok());

    let seal = Seal::minted(&journal, &a_machine());

    assert_eq!(
        std::fs::read_to_string(&kept).unwrap_or_default(),
        "not a key at all",
        "left exactly as it was"
    );
    assert_eq!(seal.opening(&sealed), sealed, "and it opens nothing");
    assert!(!original.is_empty(), "there was a key before");
}

/// A value this machine has no key for is handed back as it was read, still marked,
/// rather than dropped or guessed at.
#[test]
fn a_value_that_will_not_open_is_left_exactly_as_it_reads() {
    let journal = scratch("lost-key");
    let sealed = Seal::minted(&journal, &a_machine()).sealing(&credential(None), &a_machine());
    assert!(std::fs::remove_file(beside(&journal)).is_ok());

    let opened = Seal::kept(&journal).opening(&sealed);

    assert_eq!(opened, sealed, "unchanged, and still marked");
    assert!(as_written(&opened).contains(MARKER), "{opened:?}");
}

/// Sealed under one key, opened under another: authenticated, so it does not open.
#[test]
fn a_value_sealed_under_another_key_does_not_open() {
    let mine = scratch("mine");
    let theirs = scratch("theirs");
    let sealed = Seal::minted(&mine, &a_machine()).sealing(&credential(None), &a_machine());
    let other = Seal::minted(&theirs, &answering(Some(vec![5; 32])));

    assert_eq!(other.opening(&sealed), sealed, "it does not open");
    assert_eq!(
        other.sealing(&sealed, &a_machine()),
        sealed,
        "and writing it back does not bury it under a second key"
    );
}

/// A record somebody edited is a record that will not open, rather than one that
/// opens to something else.
#[test]
fn a_record_changed_since_it_was_written_does_not_open() {
    let journal = scratch("tampered");
    let seal = Seal::minted(&journal, &a_machine());
    let sealed = seal
        .seal("what-it-holds-now", &a_machine())
        .unwrap_or_default();

    // A byte appended: still hexadecimal, still marked, and no longer the record that
    // was written. Appended rather than altered in place, so this is one edit rather
    // than a search through the value for something to edit.
    let edited = format!("{sealed}00");

    assert!(is_sealed(&sealed), "there was a sealed value to change");
    assert!(
        seal.open(&sealed).is_some(),
        "and it opened before the change"
    );
    assert!(seal.open(&edited).is_none(), "{edited}");
}

/// Only a setting whose name says it holds a credential is sealed. A journal that
/// sealed everything would be a journal nobody could read.
#[test]
fn nothing_but_a_credential_setting_is_touched() {
    let journal = scratch("untouched");
    let seal = Seal::minted(&journal, &a_machine());
    let plain = Change {
        kind: Kind::Set {
            key: "TZ".to_owned(),
            previous: None,
            current: "Europe/Amsterdam".to_owned(),
        },
        ..credential(None)
    };
    let made = Change {
        kind: Kind::Made {
            path: "/srv/media".to_owned(),
        },
        ..credential(None)
    };

    for change in [plain, made] {
        assert_eq!(seal.sealing(&change, &a_machine()), change);
        assert_eq!(seal.opening(&change), change);
    }
}

/// A value that is not written the way a sealed one is, is not one.
#[test]
fn a_value_is_sealed_only_where_it_says_so() {
    assert!(is_sealed(&format!("{MARKER}00")));
    assert!(!is_sealed("hunter2"));
    assert!(!is_sealed(""));
}

/// Hexadecimal, or nothing. A key file somebody has typed into is a key file that
/// opens nothing, which is the safe direction.
#[test]
fn only_a_run_of_hexadecimal_reads_as_bytes() {
    assert_eq!(unhex("0f10"), Some(vec![0x0f, 0x10]));
    assert_eq!(unhex(""), Some(Vec::new()));
    assert_eq!(unhex("abc"), None, "an odd number of digits");
    assert_eq!(unhex("zz"), None, "not digits");
    assert_eq!(unhex("+f"), None, "a sign is not a digit");
}
