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
use crate::journal::{Action, Undo};

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
pub fn undo(undos: &[Undo], env_file: &Path) -> Result<(), Box<Problem>> {
    let mut beyond_reach = Vec::new();
    for undo in undos {
        match carry_out(&undo.action, env_file).map_err(|fault| Box::new(fault.problem()))? {
            Step::Done => {}
            Step::BeyondReach(resource) => beyond_reach.push(resource),
        }
    }
    if beyond_reach.is_empty() {
        Ok(())
    } else {
        Err(Box::new(needs_service(&beyond_reach)))
    }
}

/// What one undo amounted to: carried out, or beyond a filesystem-and-config
/// reversal's reach.
enum Step {
    /// The undo was carried out.
    Done,
    /// The change was made through a service, named by the resource it created,
    /// and only that service can undo it.
    BeyondReach(String),
}

/// Carry out one undo against the filesystem or the environment file.
fn carry_out(action: &Action, env_file: &Path) -> Result<Step, Fault> {
    match action {
        Action::Restore {
            key,
            value: Some(value),
        } => store::set(env_file, key, value)
            .map(|()| Step::Done)
            .map_err(Fault::Store),
        Action::Restore { key, value: None } => store::unset(env_file, key)
            .map(|()| Step::Done)
            .map_err(Fault::Store),
        Action::Delete { path } => remove(Path::new(path)).map(|()| Step::Done),
        Action::Remove { resource, .. } => Ok(Step::BeyondReach(resource.clone())),
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

/// Raised when a directory from an interrupted apply could not be removed.
pub const NOT_REMOVED: Code = Code::new("SETUP-3");

/// Raised when reversing needs the service that made a change.
pub const NEEDS_SERVICE: Code = Code::new("SETUP-4");

#[cfg(test)]
mod tests {
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
    /// `None`.
    fn restore(key: &str, value: Option<&str>) -> Undo {
        Undo {
            target: ".env".to_owned(),
            action: Action::Restore {
                key: key.to_owned(),
                value: value.map(str::to_owned),
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
    fn restoring_a_setting_writes_its_earlier_value_back() {
        let dir = scratch("restore");
        let env = dir.join(".env");
        assert!(store::set(&env, "TZ", "Pacific/Auckland").is_ok());

        assert!(undo(&[restore("TZ", Some("Europe/Amsterdam"))], &env).is_ok());

        let file = store::read(&env).unwrap_or_default();
        assert_eq!(file.get("TZ"), Some("Europe/Amsterdam"));
    }

    #[test]
    fn restoring_a_setting_that_was_not_there_removes_it() {
        let dir = scratch("unset");
        let env = dir.join(".env");
        assert!(store::set(&env, "USENET", "on").is_ok());

        assert!(undo(&[restore("USENET", None)], &env).is_ok());

        let file = store::read(&env).unwrap_or_default();
        assert_eq!(file.get("USENET"), None);
    }

    #[test]
    fn removing_a_made_directory_takes_it_off_disk() {
        let dir = scratch("rmdir");
        let made = dir.join("made");
        assert!(std::fs::create_dir_all(&made).is_ok());

        assert!(undo(&[delete(&made)], &dir.join(".env")).is_ok());

        assert!(!made.exists(), "the directory was removed");
    }

    #[test]
    fn a_directory_a_stop_never_made_is_treated_as_already_undone() {
        let dir = scratch("gone");
        let never = dir.join("never-made");

        assert!(undo(&[delete(&never)], &dir.join(".env")).is_ok());
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

        assert!(undo(&journal.rewind(), &env).is_ok());

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

        let stopped = undo(&[delete(&made)], &dir.join(".env"));

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
        let outcome = undo(&[restore("USENET", None), created], &env);

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

        let stopped = undo(&[restore("TZ", Some("Europe/Amsterdam"))], &env);

        assert!(stopped.is_err(), "the setting could not be restored");
    }

    #[test]
    fn a_setting_that_cannot_be_removed_stops_the_reversal() {
        let dir = scratch("nounset");
        // The same unwritable location, reached through the remove path this time —
        // undoing an added key that cannot be read or rewritten.
        let env = dir.join("env-is-a-directory");
        assert!(std::fs::create_dir_all(&env).is_ok());

        let stopped = undo(&[restore("USENET", None)], &env);

        assert!(stopped.is_err(), "the key could not be removed");
    }
}
