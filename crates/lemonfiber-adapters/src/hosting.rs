//! Writing a service definition and telling the manager about it.
//!
//! Translation, and no decisions. Which command should be hosted, what its
//! standing means and what to say about it are settled above the port; here a
//! definition is written in the one dialect this machine's manager reads, the
//! manager is told, and what it said comes straight back.
//!
//! The two dialects are separate files because they share no syntax, and the
//! third implementation shares nothing with either: a platform with no manager
//! answers every operation with the same refusal, which is what keeps the
//! decision about unsupported platforms in one place above rather than repeated
//! at every call site.
//!
//! Neither writes a restart policy. Both hosted commands end for a reason the
//! operator has to hear about — a volume that went, an arrangement that was
//! withdrawn — and a manager told to bring them back would bring them back past
//! the reason, once every few seconds, for as long as the machine is on.

pub mod launchd;
pub mod systemd;

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use lemonfiber_ports::hosting::{Failure, Held, Host, Hosted, Manager, Placed};
use lemonfiber_ports::process::Output;

/// A machine whose platform lemonfiber does not configure.
///
/// No derives. It carries nothing, it is held as `Arc<dyn Host>` everywhere it is
/// used, and nothing formats or copies it — so a derived implementation here would
/// be a function the coverage gate counts and no test can reach.
pub struct Unhosted;

#[async_trait]
impl Host for Unhosted {
    fn manager(&self) -> Manager {
        Manager::Unsupported
    }

    async fn place(&self, _hosted: &Hosted) -> Result<Placed, Failure> {
        Err(Failure::Unhostable)
    }

    async fn standing(&self, _name: &str) -> Result<Held, Failure> {
        Err(Failure::Unhostable)
    }

    async fn withdraw(&self, _name: &str) -> Result<Vec<PathBuf>, Failure> {
        Err(Failure::Unhostable)
    }
}

/// Write a definition where the manager reads them, making the directory if it
/// is not there yet.
///
/// Every definition is a file inside the manager's own directory, so the parent is
/// the directory to make. A path with no parent at all is the filesystem root,
/// which is there already — said as a fallback rather than as a branch, because a
/// branch nothing can reach is a line no test can cover.
fn put(at: &Path, text: &str) -> Result<(), Failure> {
    let parent = at.parent().unwrap_or(at);
    std::fs::create_dir_all(parent).map_err(|error| unwritable(at, &error))?;
    std::fs::write(at, text).map_err(|error| unwritable(at, &error))
}

/// The definition as it stands, or nothing where none is installed.
fn definition(at: &Path) -> Option<String> {
    std::fs::read_to_string(at).ok()
}

/// Take the definition away, reporting a refusal to remove it as the failure it is.
fn take(at: &Path) -> Result<(), Failure> {
    std::fs::remove_file(at).map_err(|error| unwritable(at, &error))
}

/// The failure for a definition that would not go where it belongs.
fn unwritable(at: &Path, error: &std::io::Error) -> Failure {
    Failure::Unwritable {
        at: at.to_path_buf(),
        reason: error.to_string(),
    }
}

/// The manager's own words for a refusal, preferring what it said on the error
/// channel and falling back to the rest.
fn complaint(output: &Output) -> String {
    let said = if output.stderr.trim().is_empty() {
        output.stdout.trim()
    } else {
        output.stderr.trim()
    };
    if said.is_empty() {
        format!("it exited {}", status(output))
    } else {
        said.to_owned()
    }
}

/// How a program ended, in words, since a signal leaves no number.
fn status(output: &Output) -> String {
    output
        .status
        .map_or_else(|| "on a signal".to_owned(), |code| code.to_string())
}

/// The value of one `key = value` line, trimmed, or nothing where there is none.
fn after(text: &str, key: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix(key))
        .map(|rest| rest.trim().to_owned())
        .next()
}

/// A directory of this file's own, unique to one test and to one process.
///
/// Written here rather than reached for in the app layer's fixtures: an adapter is
/// below that layer and reaching up into it would be the dependency this seam
/// exists to prevent — and the fixture is private to it in any case.
#[cfg(test)]
pub(super) fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("lemonfiber-hosting-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
    use super::{after, complaint, definition, put, status, take, Host, Unhosted};
    use lemonfiber_ports::hosting::{Failure, Hosted};
    use lemonfiber_ports::process::Output;
    use std::path::PathBuf;

    fn spoke(status: Option<i32>, stdout: &str, stderr: &str) -> Output {
        Output {
            status,
            stdout: stdout.to_owned(),
            stderr: stderr.to_owned(),
        }
    }

    fn a_command() -> Hosted {
        Hosted {
            name: "watch".to_owned(),
            program: PathBuf::from("/usr/local/bin/lemonfiber"),
            arguments: vec!["watch".to_owned()],
            output: PathBuf::from("/records/watch.log"),
            about: "guards the data location".to_owned(),
        }
    }

    #[tokio::test]
    async fn a_platform_with_no_manager_refuses_every_operation_the_same_way() {
        assert_eq!(
            Unhosted.manager(),
            lemonfiber_ports::hosting::Manager::Unsupported
        );
        assert_eq!(Unhosted.place(&a_command()).await, Err(Failure::Unhostable));
        assert_eq!(Unhosted.standing("watch").await, Err(Failure::Unhostable));
        assert_eq!(Unhosted.withdraw("watch").await, Err(Failure::Unhostable));
    }

    #[test]
    fn a_definition_is_written_read_back_and_taken_away_again() {
        let at = super::scratch("round-trip").join("nested/one.service");
        assert_eq!(definition(&at), None);
        assert_eq!(put(&at, "[Service]\n"), Ok(()));
        assert_eq!(definition(&at).as_deref(), Some("[Service]\n"));
        assert_eq!(take(&at), Ok(()));
        assert_eq!(definition(&at), None);
    }

    #[test]
    fn a_definition_that_will_not_be_written_carries_the_platforms_words() {
        let at = super::scratch("unwritable").join("one.service");
        assert!(put(&at, "[Service]\n").is_ok());
        // A directory cannot be written over as a file, which is the shape of every
        // refusal here: the platform says why, and its words travel unchanged.
        let over = at.join("beneath.service");
        assert!(matches!(
            put(&over, "x"),
            Err(Failure::Unwritable { at: refused, .. }) if refused == over
        ));
        assert!(matches!(
            take(&at.join("absent")),
            Err(Failure::Unwritable { .. })
        ));
    }

    /// The two ways writing a definition fails are different failures.
    ///
    /// One is the directory it goes in and the other is the file itself, and only
    /// the second is reached where the directory was made without complaint — so a
    /// test that only ever blocked the directory left the write's own refusal
    /// carrying words nothing had read.
    #[test]
    fn a_definition_the_file_itself_refuses_carries_the_platforms_words() {
        let dir = super::scratch("write-refused");
        let at = dir.join("one.service");
        // A directory standing where the file goes: the directory above it is made
        // without complaint, and the write is what refuses.
        let _ = std::fs::create_dir_all(&at);
        assert!(matches!(
            put(&at, "[Service]\n"),
            Err(Failure::Unwritable { at: refused, .. }) if refused == at
        ));
    }

    #[test]
    fn a_refusal_quotes_the_error_channel_first_and_the_rest_after() {
        assert_eq!(
            complaint(&spoke(Some(1), "out", "bad domain")),
            "bad domain"
        );
        assert_eq!(complaint(&spoke(Some(1), "out", "  ")), "out");
        assert_eq!(complaint(&spoke(Some(5), "", "")), "it exited 5");
        assert_eq!(complaint(&spoke(None, "", "")), "it exited on a signal");
    }

    #[test]
    fn how_a_program_ended_is_a_number_or_the_absence_of_one() {
        assert_eq!(status(&spoke(Some(0), "", "")), "0");
        assert_eq!(status(&spoke(None, "", "")), "on a signal");
    }

    #[test]
    fn the_value_after_a_key_is_the_first_one_and_nothing_where_there_is_none() {
        let text = "Description=one\nExecStart=/bin/lemonfiber watch\nExecStart=/second\n";
        assert_eq!(
            after(text, "ExecStart=").as_deref(),
            Some("/bin/lemonfiber watch")
        );
        assert_eq!(after(text, "Restart=").as_deref(), None);
    }
}
