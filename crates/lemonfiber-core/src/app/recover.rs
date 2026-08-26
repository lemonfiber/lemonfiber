//! Carrying out a failed apply's reversal.
//!
//! The recovery frame decides what to undo — [`crate::wizard::Recovery`] turns a
//! choice into the ordered undos — and this performs them. It is the doing half of
//! recovery: the deciding is pure, so a reversal is planned in a test with no disk,
//! and carried out here against a real one.
//!
//! What it reverses is what an apply writes: a setting restored to what it held,
//! and a directory apply made removed again. A change a service made — a resource
//! created through an API — is beyond its reach, and named as such rather than
//! passed over, since undoing that needs the service this reversal cannot speak to.

use std::path::{Path, PathBuf};

use crate::config::store;
use crate::error::{Code, Diagnose, Problem, Remedy, Severity};
use crate::journal::{Action, Change, Journal, Undo};
use crate::ports::service::Client as _;
use crate::repair;

use super::Ctx;

/// The change journal saved at `path`, empty where none is there or it does not
/// read.
///
/// A torn final line — a crash caught mid-write — is dropped rather than failing
/// the whole read, so a reversal still has every entry that fully landed to work
/// from; an absent or unreadable file is an empty journal, nothing to reverse.
#[must_use]
pub fn journal_at(path: &Path) -> Journal {
    let changes = std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Change>(line).ok())
        .collect();
    Journal::replay(changes)
}

/// Add what a change made to the journal a reversal reads, keeping what is already there.
///
/// Appended rather than rewritten, because the journal is shared: the first-run wizard
/// wrote what it applied, seeding wrote what it wired, and a repair adding its own must not
/// take either away. Each entry is one line, so a stop part way through costs at most the
/// line being written — which [`journal_at`] then drops as torn.
///
/// Written through the same seam every other record lemonfiber keeps goes through, so the
/// journal is created private to its owner. It holds what a value was before it changed,
/// and a record of an operator's configuration is not something to leave world-readable
/// because this one caller wrote it a different way.
///
/// Silent where it cannot be written. A repair has already changed the thing it was asked
/// to change by the time this runs, and failing the repair over the record of it would
/// report a change that did happen as one that did not.
pub fn journalled(path: &Path, changes: &[Change]) {
    if changes.is_empty() {
        return;
    }
    let mut written = std::fs::read_to_string(path).unwrap_or_default();
    if !written.is_empty() && !written.ends_with('\n') {
        written.push('\n');
    }
    for change in changes {
        if let Ok(line) = serde_json::to_string(change) {
            written.push_str(&line);
            written.push('\n');
        }
    }
    let _ = store::write(path, &written);
}

/// Put back the changes that live inside a service, answering with the ones left for
/// [`undo`] to carry out on the filesystem and the environment file.
///
/// Ahead of that reversal rather than inside it, because this is the only part that needs
/// to speak to a service — and a reversal that could not reach one would otherwise have to
/// choose between failing everything and silently skipping the part it could not do.
///
/// A service that cannot be reached leaves its changes named in the returned list, and the
/// caller reports them together. What was put back is put back either way: an operator with
/// one service down should not be left with a half-reversed repair *and* no account of
/// which half.
pub async fn reconfigured(
    ctx: &Ctx,
    undos: &[Undo],
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> (Vec<Undo>, Vec<String>) {
    let mut left = Vec::new();
    let mut unreached = Vec::new();
    for undo in undos {
        let Action::Reconfigure {
            resource,
            id,
            field,
            value,
        } = &undo.action
        else {
            left.push(undo.clone());
            continue;
        };
        let reached = match super::targets::target_named(services, project, &undo.target) {
            Some(target) => target.open(&ctx.http, ctx.filesystem.as_ref()).await,
            None => None,
        };
        let put_back = match reached {
            Some(client) => client
                .set_client_field(id, field, value.as_deref())
                .await
                .is_ok(),
            None => false,
        };
        if !put_back {
            unreached.push(format!("{resource} in {}", undo.target));
        }
    }
    (left, unreached)
}

/// Carry out a reversal, undo by undo, in the order given.
///
/// The undos come most-recent-first from [`crate::journal::Journal::rewind`], so a
/// directory comes off after the settings that named it and a child before its
/// parent — the order this walks them in.
///
/// # Errors
///
/// Stops and returns a [`Problem`] the moment a setting cannot be rewritten or a
/// directory cannot be removed — a real I/O failure where going on would only
/// compound it. A change that needs the service that made it does not stop the
/// rest: those are set aside and reported together at the end, so everything this
/// reversal can undo is undone first.
///
/// `already` carries the ones an earlier step — [`reconfigured`] — could not reach, so an
/// operator is told about every change still standing in one sentence rather than learning
/// about them a service at a time.
///
/// A setting that no longer holds what the change put there is left exactly as it is,
/// and named at the end alongside those. It is the rule a repair keeps on the way out —
/// [`crate::repair::may_put_back`] — kept on the way back: a reversal that wrote its old
/// value over one the operator has chosen since would be taking away a decision made
/// after the repair it is undoing.
///
/// The settings the operator owns are reported ahead of the services that would not
/// answer, where a reversal meets both. An unreachable service announces itself in every
/// other reading of the stack; a reversal that deliberately did not write is something an
/// operator can find out no other way.
pub fn undo(undos: &[Undo], env_file: &Path, already: Vec<String>) -> Result<(), Box<Problem>> {
    let mut beyond_reach = already;
    let mut theirs = Vec::new();
    for undo in undos {
        match carry_out(&undo.action, env_file).map_err(|fault| Box::new(fault.problem()))? {
            Step::Done => {}
            Step::BeyondReach(resource) => beyond_reach.push(resource),
            Step::TheirsNow(key) => theirs.push(key),
        }
    }
    if !theirs.is_empty() {
        return Err(Box::new(not_put_back(&theirs)));
    }
    if beyond_reach.is_empty() {
        Ok(())
    } else {
        Err(Box::new(needs_service(&beyond_reach)))
    }
}

/// What one undo amounted to: carried out, beyond a filesystem-and-config
/// reversal's reach, or the operator's to keep.
enum Step {
    /// The undo was carried out, or there was nothing left of it to carry out.
    Done,
    /// The change was made through a service, named by the resource it created,
    /// and only that service can undo it.
    BeyondReach(String),
    /// The setting holds neither what the change put there nor what putting it back
    /// would write, so somebody has chosen it since and it is left alone.
    TheirsNow(String),
}

/// Carry out one undo against the filesystem or the environment file.
fn carry_out(action: &Action, env_file: &Path) -> Result<Step, Fault> {
    match action {
        Action::Restore { key, value, wrote } => put_back(env_file, key, value.as_deref(), wrote),
        Action::Delete { path } => remove(Path::new(path)).map(|()| Step::Done),
        // Both need the service that made the change: one to delete what it created, the
        // other to put a field of it back. Neither is something the host can do, and a
        // reversal that took the second for an ordinary setting would write the field's
        // name into the environment file and leave the service exactly as it was.
        Action::Remove { resource, .. } | Action::Reconfigure { resource, .. } => {
            Ok(Step::BeyondReach(resource.clone()))
        }
    }
}

/// Put one setting back to `value`, where the change being reversed is still the
/// last thing that touched it.
///
/// Three answers, because there are three things the file can be holding. What the
/// change put there is lemonfiber's own work and comes off. What putting it back
/// would write is already there — a reversal carried out once, or one asked for
/// twice — and there is nothing left to do, which is what makes asking again
/// harmless rather than forbidden. Anything else was chosen after the change, by
/// the only other hand that reaches this file, and is left exactly as it is.
///
/// The already-back question is asked before the ownership one, and has to be: the
/// value a reversal leaves behind is by definition not the one the change wrote, so
/// asking the other way round would have every second reversal accusing the
/// operator of a change they did not make.
fn put_back(env_file: &Path, key: &str, value: Option<&str>, wrote: &str) -> Result<Step, Fault> {
    let file = store::read(env_file).map_err(Fault::Store)?;
    let holds = file.get(key);
    if holds == value {
        return Ok(Step::Done);
    }
    if !repair::may_put_back(wrote, holds).allowed() {
        return Ok(Step::TheirsNow(key.to_owned()));
    }
    match value {
        Some(value) => store::set(env_file, key, value)
            .map(|()| Step::Done)
            .map_err(Fault::Store),
        None => store::unset(env_file, key)
            .map(|()| Step::Done)
            .map_err(Fault::Store),
    }
}

/// Remove a directory apply made, treating one already gone as already undone.
///
/// Only ever an empty directory — apply records a directory the moment it makes
/// it, before anything is put inside — so a plain [`std::fs::remove_dir`] is right:
/// it removes the empty directory and refuses to walk into a populated one, so an
/// operator's own location is never emptied by a reversal. A directory a stop left
/// unmade is not there, and needs nothing done.
fn remove(path: &Path) -> Result<(), Fault> {
    match std::fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(Fault::NotRemoved {
            path: path.to_path_buf(),
            reason: err.to_string(),
        }),
    }
}

/// An I/O failure that stops a reversal: the environment file or a directory that
/// would not budge. A service-made change is not one of these — it does not stop
/// the reversal, so it is reported apart, in [`needs_service`].
enum Fault {
    /// The environment file could not be rewritten.
    Store(store::Failure),
    /// A directory could not be removed.
    NotRemoved {
        /// The directory left in place.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
}

impl Fault {
    /// The problem to report, in the words that fit what stopped the reversal.
    fn problem(&self) -> Problem {
        match self {
            Self::Store(failure) => failure.problem(),
            Self::NotRemoved { path, reason } => Problem::new(
                NOT_REMOVED,
                Severity::Error,
                "A directory from the interrupted setup could not be removed",
                "The rest of the setup was reversed; this one directory is still there. It holds nothing.",
                Remedy::new("Remove it by hand, or leave it where it is"),
            )
            .with_detail(format!("{}: {reason}", path.display())),
        }
    }
}

/// The problem naming the changes a reversal reached the end with still undone,
/// because only the service that made each can undo it.
fn needs_service(resources: &[String]) -> Problem {
    Problem::new(
        NEEDS_SERVICE,
        Severity::Error,
        "Some changes cannot be undone without the service that made them",
        "This reversal restores settings and removes directories; a resource a service was told to create is undone through that service, not here. Everything else was reversed.",
        Remedy::new("Reverse them from the service once it is reachable"),
    )
    .with_detail(resources.join(", "))
}

/// The problem naming the settings a reversal left exactly as they are, because
/// they hold neither what the change wrote nor what putting it back would write.
///
/// Every one of them is named. "Something was left alone" tells an operator that a
/// reversal was incomplete without telling them which of their settings it decided
/// was theirs, and the answer decides whether they do anything next.
///
/// What was reversed is said too, in the same sentence. A reversal that put three
/// settings back and left a fourth is not a reversal that failed, and reading only
/// the refusal would have an operator checking three settings that are already
/// right.
fn not_put_back(settings: &[String]) -> Problem {
    Problem::new(
        NOT_PUT_BACK,
        Severity::Warning,
        "Some settings hold what you chose, so they were left alone",
        "Undoing a change puts back what lemonfiber replaced, and these settings no longer hold \
         what it wrote — so they were changed after it, and writing the old value over that \
         would take away a decision you made. Everything else was put back.",
        Remedy::new("Set them back by hand if the earlier value is the one you want"),
    )
    .with_detail(settings.join(", "))
}

/// Raised when a directory from an interrupted apply could not be removed.
pub const NOT_REMOVED: Code = Code::new("SETUP-3");

/// Raised when reversing needs the service that made a change.
pub const NEEDS_SERVICE: Code = Code::new("SETUP-4");

/// Raised when a reversal would write over a setting the operator has since chosen.
pub const NOT_PUT_BACK: Code = Code::new("SETUP-9");

#[cfg(test)]
mod tests {
    use crate::test_support::a_fresh_write;
    use std::path::{Path, PathBuf};

    use super::undo;
    use crate::config::store;
    use crate::journal::{Action, Change, Journal, Kind, Undo};

    /// A scratch directory unique to this process and case, cleared first.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-recover-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
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

        assert_eq!(super::journal_at(&path).changes(), changes);
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

        super::journalled(&path, &[a_fresh_write("TORRENT", "on")]);

        assert_eq!(
            super::journal_at(&path).changes(),
            [
                a_fresh_write("USENET", "on"),
                a_fresh_write("TORRENT", "on")
            ]
        );
    }

    /// A repair that changed nothing reversible writes nothing at all — including no
    /// empty file where a reversal would then find a journal that says nothing.
    #[test]
    fn a_repair_that_changed_nothing_writes_no_journal() {
        let path = scratch("journal-none").join("journal.jsonl");

        super::journalled(&path, &[]);

        assert!(!path.exists());
    }

    /// The directory is made where it is not there yet — a repair on a stack whose
    /// configuration directory nobody has written to is still a repair worth recording.
    #[test]
    fn a_journal_is_written_where_no_directory_has_been_made_yet() {
        let path = scratch("journal-fresh").join("journal.jsonl");

        super::journalled(&path, &[a_fresh_write("USENET", "on")]);

        assert_eq!(
            super::journal_at(&path).changes(),
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

        assert_eq!(super::journal_at(&path).changes().len(), 1);
    }

    #[test]
    fn no_journal_file_reads_as_nothing_to_reverse() {
        let absent = Path::new("/lemonfiber/no/such/journal.jsonl");
        assert_eq!(super::journal_at(absent).changes().len(), 0);
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
        assert_eq!(
            store::read(&env).ok().map(|file| file.keys().len()),
            Some(0),
            "every setting was restored to absent",
        );
        assert!(!made.exists(), "the directory was removed");
    }

    #[test]
    fn a_directory_that_is_not_empty_stops_the_reversal() {
        let dir = scratch("notempty");
        let made = dir.join("populated");
        // A non-empty directory is not one apply left for reversal — removing it
        // would need to walk into contents this reversal must never touch — so it
        // is reported rather than force-removed.
        assert!(std::fs::create_dir_all(made.join("inside")).is_ok());

        let stopped = undo(&[delete(&made)], &dir.join(".env"), Vec::new());

        assert!(matches!(stopped, Err(problem) if problem.code == super::NOT_REMOVED));
        assert!(made.exists(), "and it is left where it is");
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
}
