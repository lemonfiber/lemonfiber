//! Installing a plugin the record already holds.
//!
//! Two answers, told apart by where the plugin came from. The same source again is an
//! update asked for as an install; another source under the same name is a second
//! plugin, and which of the two runs is the operator's to choose.

use std::path::{Path, PathBuf};

use crate::error::codes::plugin::{ALREADY, TWO_SOURCES};
use crate::error::{Problem, Remedy, Severity, State};

/// This plugin is installed, so what was asked for is an update, or a second plugin
/// under a name the first already holds.
///
/// **Two sources are two plugins until the operator says otherwise.** A name is what the
/// author chose, and two authors can choose one; what tells them apart is where each
/// came from. So a source other than the one the record holds is refused naming both,
/// and which of them this machine runs is left to the operator rather than decided by
/// whichever was installed second.
pub(super) fn already(held: &crate::plugin::Already, path: &Path) -> Problem {
    if !held.from.is_empty() && !same_source(&held.from, path) {
        return Problem::new(
            TWO_SOURCES,
            Severity::Error,
            format!(
                "{} is already installed from {}, and this is another source for it: {}",
                held.plugin,
                held.from,
                path.display()
            ),
            "Nothing was written. Two sources for one name are two plugins as far as anybody \
             can tell, and which of them this machine runs is yours to choose rather than this \
             command's.",
            Remedy::new(format!(
                "To run this source in its place, `lemonfiber plugin update {}`; to keep the one \
                 installed, nothing needs doing",
                path.display()
            )),
        )
        .in_state(State::Guided)
        .with_detail(format!(
            "installed: {} from {}; asked: {}",
            held.version,
            held.from,
            path.display()
        ));
    }
    Problem::new(
        ALREADY,
        Severity::Error,
        format!("{} is already installed", held.plugin),
        "Nothing was written. Installing over an installation is an update, which puts one set \
         of changes back before it applies another — doing it as an install would leave the \
         record describing one version and the machine carrying two.",
        Remedy::new(format!(
            "Run `lemonfiber plugin update` on the new source, or remove {} first",
            held.plugin
        )),
    )
    .in_state(State::Guided)
    .with_detail(format!("the record holds version {}", held.version))
}

/// Whether a source the record holds and one just named are the same place.
///
/// Compared as the directories they resolve to where both still resolve, so `./komga`,
/// the directory it names and the `plugin.toml` inside it are one source; compared as
/// written where either does not, since a source that has gone is still the one it was.
fn same_source(held: &str, path: &Path) -> bool {
    match (directory(Path::new(held)), directory(path)) {
        (Some(held), Some(path)) => held == path,
        _ => Path::new(held) == path,
    }
}

/// The directory a source names, resolved, or nothing where it no longer resolves.
fn directory(source: &Path) -> Option<PathBuf> {
    let mut at = std::fs::canonicalize(source).ok()?;
    if at.is_file() {
        at.pop();
    }
    Some(at)
}
