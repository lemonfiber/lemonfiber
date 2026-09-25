use crate::test_support::a_fresh_write;
use lemonfiber_fixtures::ports::Chance;
use std::path::Path;

use super::undo;
use crate::config::store;
use crate::journal::{Action, Change, Journal, Kind, Undo};

/// The randomness a real machine supplies, for the key the journal's credentials
/// are sealed under.
fn a_machine() -> Chance {
    Chance::cycling()
}

/// A scratch directory unique to this process and case, cleared first.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name)
}

/// An undo that restores a setting to `value`, or removes it where `value` is
/// `None`, over the value `wrote` that the change being reversed put there.
fn restore(key: &str, value: Option<&str>, wrote: &str) -> Undo {
    Undo {
        target: ".env".to_owned(),
        action: Action::Restore {
            key: key.to_owned(),
            value: value.map(str::to_owned),
            wrote: wrote.to_owned(),
        },
    }
}

/// An undo that removes a directory.
fn delete(path: &Path) -> Undo {
    Undo {
        target: path.display().to_string(),
        action: Action::Delete {
            path: path.display().to_string(),
        },
    }
}

#[test]
fn a_journal_reads_back_the_changes_that_were_written() {
    let dir = scratch("journal-read");
    let path = dir.join("journal.jsonl");
    assert!(std::fs::create_dir_all(&dir).is_ok());
    let changes = [
        a_fresh_write("USENET", "on"),
        a_fresh_write("TORRENT", "on"),
    ];
    let text = changes
        .iter()
        .map(|change| serde_json::to_string(change).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(std::fs::write(&path, text).is_ok());

    assert_eq!(
        super::journal_at(&path).unwrap_or_default().changes(),
        changes
    );
}

/// A repair adding what it changed keeps what is already there. The journal is
/// shared: the first run wrote what it applied and seeding wrote what it wired, and a
/// repair that rewrote the file would take an operator's way back to both.
#[test]
fn a_change_added_to_a_journal_keeps_what_was_already_in_it() {
    let dir = scratch("journal-append");
    let path = dir.join("journal.jsonl");
    assert!(std::fs::create_dir_all(&dir).is_ok());
    let first = serde_json::to_string(&a_fresh_write("USENET", "on")).unwrap_or_default();
    // Written without a closing newline, as a file whose last line was the last thing
    // anybody wrote to it would be.
    assert!(std::fs::write(&path, first).is_ok());

    assert!(super::journalled(&path, &[a_fresh_write("TORRENT", "on")], &a_machine()).is_ok());

    assert_eq!(
        super::journal_at(&path).unwrap_or_default().changes(),
        [
            a_fresh_write("USENET", "on"),
            a_fresh_write("TORRENT", "on")
        ]
    );
}

/// The file itself stays inside the bound, not merely the reading of it. A record
/// trimmed on the way in would go on growing on disk, and the horizon would be a
/// claim about what is shown rather than about what is kept.
#[test]
fn a_record_written_past_the_bound_leaves_the_bound_on_disk() {
    let dir = scratch("journal-bound");
    let path = dir.join("journal.jsonl");
    assert!(std::fs::create_dir_all(&dir).is_ok());

    // One run per stamp, written one run at a time the way a machine accumulates them.
    for stamp in 0..=crate::journal::RUNS_KEPT {
        let recorded = super::journalled(
            &path,
            &[crate::journal::Change {
                at: stamp.to_string(),
                ..a_fresh_write("PUID", "1000")
            }],
            &a_machine(),
        );
        assert!(recorded.is_ok(), "{recorded:?}");
    }

    let held = super::journal_at(&path).unwrap_or_default();
    assert_eq!(
        crate::journal::runs(held.changes()),
        crate::journal::RUNS_KEPT,
        "the bound is what the file holds"
    );
    assert_eq!(
        held.changes().first().map(|change| change.at.as_str()),
        Some("1"),
        "and the oldest run went, rather than the newest"
    );
}

/// A repair that changed nothing reversible writes nothing at all — including no
/// empty file where a reversal would then find a journal that says nothing.
#[test]
fn a_repair_that_changed_nothing_writes_no_journal() {
    let path_dir = scratch("journal-none");
    let path = path_dir.join("journal.jsonl");

    assert!(super::journalled(&path, &[], &a_machine()).is_ok());

    assert!(!path.exists());
}

/// The directory is made where it is not there yet — a repair on a stack whose
/// configuration directory nobody has written to is still a repair worth recording.
#[test]
fn a_journal_is_written_where_no_directory_has_been_made_yet() {
    let path_dir = scratch("journal-fresh");
    let path = path_dir.join("journal.jsonl");

    assert!(super::journalled(&path, &[a_fresh_write("USENET", "on")], &a_machine()).is_ok());

    assert_eq!(
        super::journal_at(&path).unwrap_or_default().changes(),
        [a_fresh_write("USENET", "on")]
    );
}

#[test]
fn a_torn_final_line_is_dropped_and_the_rest_kept() {
    let dir = scratch("journal-torn");
    let path = dir.join("journal.jsonl");
    assert!(std::fs::create_dir_all(&dir).is_ok());
    // One entry that fully landed and one half-written — a crash mid-append.
    let good = serde_json::to_string(&a_fresh_write("USENET", "on")).unwrap_or_default();
    assert!(std::fs::write(&path, format!("{good}\n{{ torn")).is_ok());

    assert_eq!(
        super::journal_at(&path).unwrap_or_default().changes().len(),
        1
    );
}

#[test]
fn no_journal_file_reads_as_nothing_to_reverse() {
    let absent = Path::new("/lemonfiber/no/such/journal.jsonl");
    assert_eq!(
        super::journal_at(absent)
            .unwrap_or_default()
            .changes()
            .len(),
        0
    );
}

#[test]
fn restoring_a_setting_writes_its_earlier_value_back() {
    let dir = scratch("restore");
    let env = dir.join(".env");
    assert!(store::set(&env, "TZ", "Pacific/Auckland").is_ok());

    assert!(undo(
        &[restore("TZ", Some("Europe/Amsterdam"), "Pacific/Auckland")],
        &env,
        Vec::new()
    )
    .is_ok());

    let file = store::read(&env).unwrap_or_default();
    assert_eq!(file.get("TZ"), Some("Europe/Amsterdam"));
}

#[test]
fn restoring_a_setting_that_was_not_there_removes_it() {
    let dir = scratch("unset");
    let env = dir.join(".env");
    assert!(store::set(&env, "USENET", "on").is_ok());

    assert!(undo(&[restore("USENET", None, "on")], &env, Vec::new()).is_ok());

    let file = store::read(&env).unwrap_or_default();
    assert_eq!(file.get("USENET"), None);
}

#[test]
fn removing_a_made_directory_takes_it_off_disk() {
    let dir = scratch("rmdir");
    let made = dir.join("made");
    assert!(std::fs::create_dir_all(&made).is_ok());

    assert!(undo(&[delete(&made)], &dir.join(".env"), Vec::new()).is_ok());

    assert!(!made.exists(), "the directory was removed");
}

#[test]
fn a_directory_a_stop_never_made_is_treated_as_already_undone() {
    let dir = scratch("gone");
    let never = dir.join("never-made");

    assert!(undo(&[delete(&never)], &dir.join(".env"), Vec::new()).is_ok());
}

#[test]
fn a_full_rollback_restores_the_settings_and_removes_the_directory() {
    let dir = scratch("full");
    let env = dir.join(".env");
    let made = dir.join("data");
    // The state a failed apply leaves: two settings written over nothing, and a
    // directory made. Rewinding the journal that recorded them and carrying it
    // out returns the machine to before the apply.
    assert!(store::set(&env, "USENET", "on").is_ok());
    assert!(store::set(&env, "TORRENT", "on").is_ok());
    assert!(std::fs::create_dir_all(&made).is_ok());

    let write = |key: &str| Change {
        at: "t".to_owned(),
        operation: "apply".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: None,
            current: "on".to_owned(),
        },
    };
    let mut journal = Journal::new();
    journal.record(Change {
        at: "t".to_owned(),
        operation: "apply".to_owned(),
        target: made.display().to_string(),
        kind: Kind::Made {
            path: made.display().to_string(),
        },
    });
    journal.record(write("USENET"));
    journal.record(write("TORRENT"));

    assert!(undo(&journal.rewind(), &env, Vec::new()).is_ok());

    // Read back through a readable file, so a read failure could not pass this
    // off as "no settings" — every setting is restored to absent, the directory
    // gone.
    //
    // Asked of the settings rather than of how many keys the file holds. Writing
    // it is what records the build that wrote it, so a file every setting has
    // been taken out of still holds that marker and counting would read it as a
    // setting that survived.
    assert_eq!(
        store::read(&env)
            .ok()
            .map(|file| ["USENET", "TORRENT"].map(|key| file.get(key).is_some())),
        Some([false, false]),
        "every setting was restored to absent",
    );
    assert!(!made.exists(), "the directory was removed");
}

/// A version pin is left standing and named, not quietly skipped. Nothing here
/// moves a version, and a reversal that reported success over one would tell an
/// operator their stack is on a release it is not.
#[test]
fn a_version_move_is_left_standing_and_reported() {
    let env_dir = scratch("repin");
    let env = env_dir.join(".env");
    let carried = super::carrying_out(
        &[Undo {
            target: "sonarr".to_owned(),
            action: Action::Repin {
                previous: "4.0.15".to_owned(),
                current: "4.1.0".to_owned(),
            },
        }],
        &env,
        Vec::new(),
    )
    .ok();
    assert_eq!(
        carried.map(|carried| (carried.done.len(), carried.beyond_reach)),
        Some((0, vec!["version 4.0.15".to_owned()]))
    );
}

/// The undo of a region an install wrote: taken out where it is still what was
/// written, left and named where somebody has edited it, and a failure — not a
/// quiet success — where its file cannot be written.
#[test]
fn a_region_is_taken_out_left_where_edited_and_refused_where_unwritable() {
    let dir = scratch("withdraw");
    let file = dir.join("Caddyfile");
    let env = dir.join(".env");
    let withdraw = |at: &Path, body: &str| Undo {
        target: at.display().to_string(),
        action: Action::Withdraw {
            path: at.display().to_string(),
            key: "config/caddy/Caddyfile".to_owned(),
            owner: "plugin komga".to_owned(),
            written: crate::materialised::checksum(body.as_bytes()),
        },
    };
    assert!(std::fs::create_dir_all(&dir).is_ok());
    assert!(std::fs::write(
        &file,
        crate::region::put("watch\n", "plugin komga", "komga\n")
    )
    .is_ok());

    let edited = super::carrying_out(&[withdraw(&file, "something else\n")], &env, Vec::new());
    assert_eq!(
        edited.ok().map(|carried| carried.theirs),
        Some(vec![file.display().to_string()]),
        "a region that is not what was written is named and left"
    );

    let taken = super::carrying_out(&[withdraw(&file, "komga\n")], &env, Vec::new());
    assert_eq!(taken.ok().map(|carried| carried.done.len()), Some(1));
    assert_eq!(
        std::fs::read_to_string(&file).ok().as_deref(),
        Some("watch\n")
    );

    let unreadable = super::carrying_out(&[withdraw(&dir, "komga\n")], &env, Vec::new());
    assert!(
        matches!(unreadable, Err(problem) if problem.code == super::NOT_WITHDRAWN),
        "a file that cannot be read back is a region that could not be taken out"
    );
}

/// A directory that will not empty is neither an I/O failure nor something to
/// force. Two plugins keep their documents in one directory and the first of them
/// made it, so taking it would take the second's with it — it is named and left
/// where it is, and the rest of the reversal carries on.
#[test]
fn a_directory_holding_something_else_is_named_and_left_where_it_is() {
    let dir = scratch("notempty");
    let made = dir.join("populated");
    assert!(std::fs::create_dir_all(made.join("inside")).is_ok());
    let env = dir.join(".env");

    let carried = super::carrying_out(&[delete(&made)], &env, Vec::new()).ok();

    assert_eq!(
        carried.map(|carried| (carried.done.len(), carried.still_holding)),
        Some((0, vec![made.display().to_string()])),
        "named, rather than counted among what went back"
    );
    assert!(made.exists(), "and it is left where it is");
    assert!(
        matches!(
            undo(&[delete(&made)], &env, Vec::new()),
            Err(problem) if problem.code == super::STILL_HOLDING
        ),
        "and a reversal that has to refuse over what it left says which directory"
    );
}

/// A directory the machine itself refuses to remove still stops the reversal, which
/// is the whole difference from the one above: an empty directory that will not go
/// is a machine not doing as it is told, and carrying on would only compound it.
#[test]
fn a_directory_the_machine_refuses_to_remove_stops_the_reversal() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = scratch("refused");
    let holding = dir.join("holding");
    let made = holding.join("made");
    assert!(std::fs::create_dir_all(&made).is_ok());
    // An empty directory, refused by the one thing that can refuse an empty one:
    // the parent it sits in is not writable, so removing the entry is not allowed.
    let locked = std::fs::set_permissions(&holding, std::fs::Permissions::from_mode(0o500));

    let stopped = undo(&[delete(&made)], &dir.join(".env"), Vec::new());

    let _ = std::fs::set_permissions(&holding, std::fs::Permissions::from_mode(0o700));
    assert!(locked.is_ok(), "the parent was made unwritable");
    assert!(matches!(stopped, Err(problem) if problem.code == super::NOT_REMOVED));
    assert!(made.exists(), "and the directory is left where it is");
}

#[test]
fn a_service_made_change_is_reported_at_the_end_without_stopping_the_rest() {
    let dir = scratch("service");
    let env = dir.join(".env");
    assert!(store::set(&env, "USENET", "on").is_ok());
    let created = Undo {
        target: "sonarr".to_owned(),
        action: Action::Remove {
            resource: "downloadclient".to_owned(),
            id: "3".to_owned(),
        },
    };

    // A setting to reverse and a service resource that only the service can
    // undo: the setting is still reversed, and the service resource is reported
    // at the end rather than stopping the reversible work before it.
    let outcome = undo(&[restore("USENET", None, "on"), created], &env, Vec::new());

    assert!(matches!(outcome, Err(problem) if problem.code == super::NEEDS_SERVICE));
    let file = store::read(&env).unwrap_or_default();
    assert_eq!(file.get("USENET"), None, "the setting was still reversed");
}

#[test]
fn a_setting_that_cannot_be_rewritten_stops_the_reversal() {
    let dir = scratch("noenv");
    // The environment file's location is a directory, so restoring a setting to
    // it cannot read or write it.
    let env = dir.join("env-is-a-directory");
    assert!(std::fs::create_dir_all(&env).is_ok());

    let stopped = undo(
        &[restore("TZ", Some("Europe/Amsterdam"), "Pacific/Auckland")],
        &env,
        Vec::new(),
    );

    assert!(stopped.is_err(), "the setting could not be restored");
}

/// The port a repair moved, as the journal records it: what it was, and what
/// the repair put there.
fn moved_the_port(previous: &str, current: &str) -> Change {
    Change {
        at: "t".to_owned(),
        operation: crate::repair::OPERATION.to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: "QBITTORRENT_PORT".to_owned(),
            previous: Some(previous.to_owned()),
            current: current.to_owned(),
        },
    }
}

/// What the environment file holds for that port now.
fn port(env: &Path) -> Option<String> {
    store::read(env)
        .unwrap_or_default()
        .get("QBITTORRENT_PORT")
        .map(str::to_owned)
}

/// A reversal asked for twice does not put its value back over one the operator
/// chose in between.
///
/// The sequence is the whole of it: a repair moves the port, the operator undoes
/// it, the operator then sets a third value deliberately, and asks to undo again.
/// The second reversal has the same journal to read and the same value to write,
/// and writing it would take away a decision that was made after the repair was
/// already reversed.
///
/// Both halves are asserted, because a reversal that refused everything would
/// pass half of this: the first one is carried out and the port is the repair's
/// old value, and only the second is refused.
#[test]
fn a_second_reversal_does_not_write_over_what_the_operator_set() {
    let dir = scratch("undo-twice");
    let env = dir.join(".env");
    assert!(store::set(&env, "QBITTORRENT_PORT", "51413").is_ok());
    let undos = crate::repair::undoing(&[moved_the_port("6881", "51413")]);

    // The operator watches the repair and asks for it back.
    assert!(
        undo(&undos, &env, Vec::new()).is_ok(),
        "the first is carried out"
    );
    assert_eq!(
        port(&env).as_deref(),
        Some("6881"),
        "the repair is reversed"
    );

    // Then they choose a third value themselves, which is theirs.
    assert!(store::set(&env, "QBITTORRENT_PORT", "49152").is_ok());
    let again = undo(&undos, &env, Vec::new());

    assert!(
        matches!(&again, Err(problem) if problem.code == super::NOT_PUT_BACK),
        "{again:?}"
    );
    assert_eq!(
        port(&env).as_deref(),
        Some("49152"),
        "the value the operator set stands"
    );
}

/// A credential whose record will not open is named and left alone, rather than put
/// back as the text the record now reads as.
///
/// Beside the drift refusal above because the two answers must not be confused: that
/// one is about a value the operator chose, this one is about a value nobody can read
/// any more. Writing the sealed text into the settings file would report the setting
/// restored and leave whatever authenticates with it holding a line of hexadecimal.
#[test]
fn a_credential_whose_record_will_not_open_is_named_rather_than_written() {
    let dir = scratch("still-sealed");
    let env = dir.join(".env");
    assert!(store::set(&env, "INDEXER_APIKEY", "chosen-since").is_ok());

    let refused = undo(
        &[restore(
            "INDEXER_APIKEY",
            Some("sealed:1:00"),
            "sealed:1:11",
        )],
        &env,
        Vec::new(),
    );

    assert!(
        matches!(&refused, Err(problem) if problem.code == super::NOT_OPENED),
        "{refused:?}"
    );
    let file = store::read(&env).unwrap_or_default();
    assert_eq!(
        file.get("INDEXER_APIKEY"),
        Some("chosen-since"),
        "and the setting is left exactly as it stands"
    );
}

/// The same reversal asked for twice with nothing changed in between is the same
/// answer twice, having written nothing the second time.
///
/// A reversal is not consumed by being carried out, so an operator whose request
/// was cut off part way can ask again — and what makes that safe is that there is
/// nothing left to do rather than that nobody may ask.
#[test]
fn a_reversal_asked_for_twice_over_puts_the_same_value_back_once() {
    let dir = scratch("undo-again");
    let env = dir.join(".env");
    assert!(store::set(&env, "QBITTORRENT_PORT", "51413").is_ok());
    let undos = crate::repair::undoing(&[moved_the_port("6881", "51413")]);

    assert!(undo(&undos, &env, Vec::new()).is_ok());
    assert!(undo(&undos, &env, Vec::new()).is_ok(), "and again");

    assert_eq!(port(&env).as_deref(), Some("6881"));
}

#[test]
fn a_setting_that_cannot_be_removed_stops_the_reversal() {
    let dir = scratch("nounset");
    // The same unwritable location, reached through the remove path this time —
    // undoing an added key that cannot be read or rewritten.
    let env = dir.join("env-is-a-directory");
    assert!(std::fs::create_dir_all(&env).is_ok());

    let stopped = undo(&[restore("USENET", None, "on")], &env, Vec::new());

    assert!(stopped.is_err(), "the key could not be removed");
}

/// A line before the last that is not a change makes the journal one this build cannot
/// read, rather than a history with that change missing from it.
#[test]
fn a_damaged_entry_before_the_last_is_refused_rather_than_skipped() {
    let dir = scratch("journal-damaged");
    let path = dir.join("journal.jsonl");
    assert!(std::fs::create_dir_all(&dir).is_ok());
    let good = serde_json::to_string(&a_fresh_write("USENET", "on")).unwrap_or_default();
    let written = format!("{{ not a change }}\n{good}\n");
    assert!(std::fs::write(&path, &written).is_ok());

    let read = super::journal_at(&path);
    assert!(
        matches!(&read, Err(store::Failure::Unreadable { reason, .. }) if reason.contains("entry 1")),
        "got: {read:?}"
    );
}

/// A journal this build cannot read is not written over: the new entries are refused
/// with it and the file is left as it was.
#[test]
fn a_journal_that_cannot_be_read_is_not_written_over() {
    let dir = scratch("journal-kept");
    let path = dir.join("journal.jsonl");
    assert!(std::fs::create_dir_all(&dir).is_ok());
    let good = serde_json::to_string(&a_fresh_write("USENET", "on")).unwrap_or_default();
    let written = format!("{{ not a change }}\n{good}\n");
    assert!(std::fs::write(&path, &written).is_ok());

    let refused = super::journalled(&path, &[a_fresh_write("TORRENT", "on")], &a_machine());
    assert!(refused.is_err(), "the record was refused");
    assert_eq!(
        std::fs::read_to_string(&path).ok(),
        Some(written),
        "and the journal is as it was"
    );
}

/// A journal that is there and cannot be opened is refused, not read as empty.
#[test]
fn a_journal_that_cannot_be_opened_is_refused_rather_than_read_as_empty() {
    let dir = scratch("journal-directory");
    let path = dir.join("journal.jsonl");
    assert!(std::fs::create_dir_all(path.join("held")).is_ok());
    assert!(matches!(
        super::journal_at(&path),
        Err(store::Failure::Unreadable { .. })
    ));
}

/// A change made and not recorded is said as that: it stands, and it cannot be put back.
#[test]
fn a_change_made_and_not_recorded_says_it_stands() {
    let problem = super::unrecorded(
        "The update",
        &store::Failure::NotWritten {
            path: Path::new("/journal.jsonl").to_path_buf(),
            reason: "no space".to_owned(),
        },
    );
    assert!(
        problem.summary.contains("The update was done"),
        "{problem:?}"
    );
    assert!(problem.meaning.contains("The change stands"), "{problem:?}");
    assert!(problem.cause.is_some_and(|cause| cause.detail.is_some()));
}
