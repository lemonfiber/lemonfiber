//! Reading and changing settings on disk.
//!
//! One setting at a time, in place, leaving the file otherwise exactly as it
//! was. The operator's own edits and comments are the reason this is not simply
//! a serialised struct.
//!
//! # Why there is no checksum of this file
//!
//! The stack directory is protected by a record of what lemonfiber last wrote to
//! each file, so an edit can be told from a version that has not been upgraded yet.
//! This file is not, and the difference is deliberate rather than an omission.
//!
//! Nothing here ever writes the file whole. A change rewrites one line, and every
//! other line — comment, blank, a setting this build has never heard of — survives
//! byte for byte, which [`super::env`] guarantees and a test below holds it to. So
//! there is no version of this file that lemonfiber replaces with its own, and
//! therefore nothing for a whole-file comparison to protect.
//!
//! What *can* be lost is one setting's value, and that is guarded by content rather
//! than by a remembered baseline: a change is read against what the file holds at
//! the moment it is proposed, shown beside what would replace it with both sides
//! withheld exactly as `config show` withholds them, and a consequential one is not
//! written until the operator says so. See [`crate::reconfigure::Review`].
//!
//! A checksum here would be the wrong instrument twice over. It would report a
//! difference for a comment somebody added, which is not a difference in any setting;
//! and having reported one it could not say which setting, because a file of lines
//! has no field for a checksum to name.

use std::path::{Path, PathBuf};

use thiserror::Error;

use super::env::{is_one_line, EnvFile};
use crate::error::{Diagnose, Problem, Remedy, Severity, State};

/// Withholding a credential from text that has no field names to read.
///
/// The rule for prose — an error's detail, a condition, a line quoted back. Which
/// settings are *displayed*, where there are names to read, is decided by the
/// allow-list in [`super::display`] instead: a keyword rule cannot answer about a
/// name nobody has thought of yet, and that is the name that leaks. A file's line has
/// names to read, so it goes through `withheld_by` with that same list.
pub use crate::error::withheld::{is_secret, withheld, withheld_by, withheld_text, REDACTED};

/// The setting recording which lemonfiber last wrote this file.
///
/// Kept in the settings file itself rather than beside it, because it is a fact
/// about that file and has to travel with it — a marker in a second file is one a
/// restore, a copy to another machine, or an operator moving their configuration
/// by hand leaves behind, and a marker that goes missing reads as permission.
///
/// Deliberately not on [`super::display`]'s list. That list is the settings an
/// operator sets and lemonfiber writes on their behalf; this is bookkeeping the
/// store stamps for itself, and listing it would offer it as something to change.
/// The two version numbers reach whoever needs them through the refusal that is
/// about them, which is the moment they mean anything.
pub(crate) const WRITTEN_BY_KEY: &str = "LEMONFIBER_CONFIG_VERSION";

/// The build doing the writing, which is what a marker is compared against.
const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// One setting, as it is safe to display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The setting's name.
    pub key: String,
    /// Its value, or a note that it is set but withheld.
    pub value: String,
    /// Whether the value is withheld rather than displayed.
    pub secret: bool,
}

/// Read the configuration file, or an empty one where none has been written.
///
/// A missing file is not a failure: it is what a machine looks like before
/// setup has run, and reading it should say "nothing is configured" rather than
/// refuse.
///
/// # Errors
///
/// Returns [`Failure`] when a file exists and cannot be read.
pub fn read(path: &Path) -> Result<EnvFile, Failure> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(EnvFile::parse(&text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(EnvFile::default()),
        Err(err) => Err(Failure::Unreadable {
            path: path.to_path_buf(),
            reason: err.to_string(),
        }),
    }
}

/// Change one setting, leaving the rest of the file as it was.
///
/// # Errors
///
/// Returns [`Failure`] when the file cannot be read or written, or when the key
/// or the value spans more than one line.
pub fn set(path: &Path, key: &str, value: &str) -> Result<(), Failure> {
    // Before the file is read, so a refused value cannot move the marker or
    // rewrite the file on its way to being turned down.
    if !is_one_line(key) || !is_one_line(value) {
        return Err(Failure::SpansLines {
            path: path.to_path_buf(),
            key: key.to_owned(),
        });
    }
    let mut file = read(path)?;
    refuse_if_newer(path, &file)?;
    file.set(key, value);
    stamped(&mut file);
    write(path, &file.render())
}

/// Remove a setting, restoring the file to not having it — what undoes an added
/// key on a rolled-back apply.
///
/// # Errors
///
/// Returns [`Failure`] when the file cannot be read or written.
pub fn unset(path: &Path, key: &str) -> Result<(), Failure> {
    let mut file = read(path)?;
    refuse_if_newer(path, &file)?;
    file.remove(key);
    stamped(&mut file);
    write(path, &file.render())
}

/// Refuse to change settings a newer lemonfiber wrote.
///
/// A newer build may have written keys this one has never heard of, and keys it
/// reads may have changed what they mean. Rewriting such a file would not lose
/// those lines — the file is held as the lines it is made of, so everything this
/// build does not recognise survives untouched — but it would stamp this older
/// build over a file it does not understand, and an operator who downgraded to
/// test something would have no way back to the configuration they had.
///
/// So the modification is refused and the file is left exactly as it was. Reading
/// is not refused with it: an older build that cannot safely *change* this file can
/// still say what is in it and what it is running, and an operator who has just
/// been refused needs precisely that.
///
/// A file with no marker is one written before this was recorded, or by hand. It is
/// changed, and gains a marker in the doing — refusing everything of unknown
/// provenance would refuse every configuration written before this existed.
fn refuse_if_newer(path: &Path, file: &EnvFile) -> Result<(), Failure> {
    let wrote = file.get(WRITTEN_BY_KEY).unwrap_or_default();
    if crate::version::Version::is_newer(wrote, RUNNING) {
        return Err(Failure::TooNew {
            path: path.to_path_buf(),
            wrote: wrote.to_owned(),
            running: RUNNING.to_owned(),
        });
    }
    Ok(())
}

/// Record this build as the one that last wrote the file.
///
/// Every change goes through [`set`] or [`unset`], so stamping in both is stamping
/// on every path that writes settings — setup, reconfiguration, restore, seeding
/// and a rollback putting a value back all arrive here.
fn stamped(file: &mut EnvFile) {
    file.set(WRITTEN_BY_KEY, RUNNING);
}

/// Write `text` to `path`, creating the directory for it where needed and
/// keeping both private to their owner.
///
/// The one place a small lemonfiber-owned file is put on disk, so the wizard's
/// progress and change journal land the same way a setting does — and report the
/// same [`Failure::NotWritten`] where they cannot. Every file that lands here may
/// hold a credential — the indexer key, the Usenet password, the VPN private key
/// — so where the platform has the notion the directory is created `0700` and the
/// file tightened to `0600`: another user on the same machine, the common shape
/// of a self-hosted host, must not be able to read what setup wrote.
///
/// Creating the directory and writing the file are one operation as far as the
/// operator is concerned, so they share one failure rather than two that say the
/// same thing. Written with `if let` rather than `map_err`, and without a block
/// around the directory: a closure is a function of its own for coverage
/// purposes, and one that only runs on failure is a symbol no passing test
/// reaches in every build of this crate. The private-mode step is folded into the
/// write's own result for the same reason — one failure path, already exercised,
/// rather than a second that only a chmod refusal on a just-written file reaches.
/// A path with no usable parent — a filesystem root, or a bare relative name whose
/// parent is the empty string — is written in the current directory rather than
/// under `create_dir_all("")`.
///
/// # Errors
///
/// Returns [`Failure::NotWritten`] where the directory could not be created or
/// the file could not be written.
pub(crate) fn write(path: &Path, text: &str) -> Result<(), Failure> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if let Err(err) = make_private_dir(parent) {
        return Err(unwritable(path, &err));
    }
    if let Err(err) = write_owner_only(path, text).and_then(|()| make_private(path)) {
        return Err(unwritable(path, &err));
    }
    Ok(())
}

/// Write `text`, creating the file owner-only from the outset where the platform
/// tracks a file mode, so a secret is never even briefly world-readable in the gap
/// between creation and tightening. `mode` applies only to a file this creates; an
/// existing one keeps its mode until the [`make_private`] that follows corrects it.
#[cfg(unix)]
fn write_owner_only(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())
}

/// Where the platform has no owner-only mode to set at creation, an ordinary write.
#[cfg(not(unix))]
fn write_owner_only(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}

/// Create the configuration directory, private to its owner where the platform
/// tracks ownership. The mode is set as the directory is created, so an existing
/// one — a parent like `~/.config` this does not own — is left exactly as it was.
#[cfg(unix)]
fn make_private_dir(parent: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
}

/// Elsewhere there is no owner-only notion to honour, so this is an ordinary
/// recursive create.
#[cfg(not(unix))]
fn make_private_dir(parent: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(parent)
}

/// Tighten a just-written file to its owner alone. Applied every write, so a file
/// left `0644` by an earlier version is corrected the next time it is touched.
#[cfg(unix)]
fn make_private(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

/// A no-op where the platform has no owner-only file mode to set.
#[cfg(not(unix))]
fn make_private(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// The failure for a location that would not take what was written to it.
fn unwritable(path: &Path, err: &std::io::Error) -> Failure {
    Failure::NotWritten {
        path: path.to_path_buf(),
        reason: err.to_string(),
    }
}

/// Every setting, with secrets withheld.
///
/// [`WRITTEN_BY_KEY`] is left out rather than withheld. It is the one line in this
/// file that is not a setting — bookkeeping the store stamps for itself — and the
/// two ways of listing it are both wrong: shown, it reads as something an operator
/// may set, and withheld, it reads as a credential they are not being trusted with.
/// The version numbers it exists for reach them through the refusal that is about
/// them, which is the only moment they mean anything.
#[must_use]
pub fn shown(file: &EnvFile) -> Vec<Shown> {
    file.keys()
        .into_iter()
        .filter(|key| *key != WRITTEN_BY_KEY)
        .map(|key| showing(key, file.get(key).unwrap_or_default()))
        .collect()
}

/// One setting as it is safe to display: a name nobody has vouched for keeps its
/// name and loses its value.
///
/// Decided by [`super::display::in_full`] rather than by reading the name for words
/// that sound like a credential. A keyword rule answers about the names somebody
/// thought of; this surface serves whatever is in the operator's file, and the
/// setting that leaks is the one nobody thought of. So the default is to withhold and
/// the exception costs somebody a sentence on the list.
///
/// A pair rather than a whole file, because the settings a run is about to write
/// are shown before there is a file holding them — and a review that redacted by
/// its own rule would be a second rule to keep in step with this one.
#[must_use]
pub fn showing(key: &str, value: &str) -> Shown {
    let displayed = super::display::in_full(key);
    Shown {
        key: key.to_owned(),
        // An empty setting reads as empty either way. A credential that is not set and
        // one that is are different faults, and saying "(set, not shown)" about a blank
        // would report the wrong one.
        value: match (displayed, value.is_empty()) {
            (_, true) => String::new(),
            (true, false) => super::display::without_credentials(value),
            (false, false) => REDACTED.to_owned(),
        },
        secret: !displayed,
    }
}

/// Configuration could not be read or changed.
#[derive(Debug, Error)]
pub enum Failure {
    /// A configuration file exists and could not be read.
    #[error("the configuration at {path} could not be read: {reason}")]
    Unreadable {
        /// The file, in full.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// Configuration could not be written.
    #[error("the configuration at {path} could not be written: {reason}")]
    NotWritten {
        /// The file, in full.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// There is nowhere to keep configuration.
    #[error("no configuration file has been chosen")]
    Nowhere,
    /// A setting's key or value carries a line break, which would write further
    /// settings rather than one setting containing it.
    #[error("the value offered for {key} spans more than one line, so writing it to {path} would write settings nobody asked for")]
    SpansLines {
        /// The file, in full.
        path: PathBuf,
        /// The setting that was being changed.
        key: String,
    },
    /// The configuration was written by a newer lemonfiber than the one running.
    #[error("the configuration at {path} was written by lemonfiber {wrote} and this is {running}")]
    TooNew {
        /// The file, in full.
        path: PathBuf,
        /// The version that wrote it.
        wrote: String,
        /// The version being asked to change it.
        running: String,
    },
}

pub use crate::error::codes::config::CONFIG_UNREADABLE;

pub use crate::error::codes::config::CONFIG_NOT_WRITTEN;

pub(crate) use crate::error::codes::config::CONFIG_NOWHERE;

pub(crate) use crate::error::codes::config::CONFIG_TOO_NEW;

pub(crate) use crate::error::codes::config::CONFIG_SPANS_LINES;

impl Diagnose for Failure {
    fn problem(&self) -> Problem {
        match self {
            Self::Unreadable { path, reason } => Problem::new(
                CONFIG_UNREADABLE,
                Severity::Error,
                format!("Your settings at {} could not be read", path.display()),
                "Nothing has been changed. lemonfiber will not guess at settings it cannot read, because guessing wrong here starts the wrong things.",
                Remedy::new("Check that the file is readable"),
            )
            .in_state(State::Guided)
            .with_detail(reason.clone()),
            Self::NotWritten { path, reason } => Problem::new(
                CONFIG_NOT_WRITTEN,
                Severity::Error,
                format!("Your settings at {} could not be saved", path.display()),
                "The change was not made. Your existing settings are untouched.",
                Remedy::new("Check that the location is writable and has space"),
            )
            .in_state(State::Guided)
            .with_detail(reason.clone()),
            Self::Nowhere => Problem::new(
                CONFIG_NOWHERE,
                Severity::Error,
                "lemonfiber has not been set up on this machine yet",
                "There is nowhere to keep settings until setup has chosen a location for them.",
                Remedy::new("Run setup").with_detail("lemonfiber setup"),
            )
            .in_state(State::Guided),
            Self::SpansLines { path, key } => Problem::new(
                CONFIG_SPANS_LINES,
                Severity::Error,
                format!("The value for {key} could not be saved"),
                "Nothing has been changed. A settings file is one setting per line, so a value with a line break in it would not be saved as that value — it would be saved as that setting and then whatever the rest of the text spells, which the stack would run as settings you never chose.",
                Remedy::new("Check where this value came from, and set it to a single line"),
            )
            .in_state(State::Guided)
            .with_detail(format!("the settings are at {}", path.display())),
            Self::TooNew {
                path,
                wrote,
                running,
            } => Problem::new(
                CONFIG_TOO_NEW,
                Severity::Error,
                format!("Your settings were written by lemonfiber {wrote}, and this is {running}"),
                "Nothing has been changed. An older lemonfiber writing over settings a newer one wrote would leave you with a file neither version can make sense of, and no way back to the one you had.",
                Remedy::new(format!("Run this with lemonfiber {wrote} or newer"))
                    .with_detail("lemonfiber update self"),
            )
            .in_state(State::Guided)
            .with_detail(format!("the settings are at {}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests;
