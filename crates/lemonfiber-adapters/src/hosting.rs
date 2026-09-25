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
pub(super) fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name)
}

#[cfg(test)]
mod tests;
