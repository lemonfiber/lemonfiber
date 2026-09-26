//! Reading and writing the change journal a reversal reads.
//!
//! Apart from the reversal itself because the two answer different questions and are
//! asked from different places: every command that changes something records it here,
//! and only a reversal reads what is recorded to put it back.

use std::path::Path;

use crate::config::store;
use crate::error::{Diagnose as _, Problem, Remedy, Severity};
use crate::journal::{kept, Change, Journal, Seal};
use crate::ports::random::Random;

/// The change journal saved at `path`, empty where none is there.
///
/// A torn final line — a crash caught mid-write by a build that wrote the journal in
/// place — is dropped rather than failing the whole read, so a reversal still has
/// every entry that fully landed to work from. Anything else that does not read is a
/// journal this build cannot read, and it is refused rather than read as the entries
/// around it: a reading that skipped a line would be a history with a change missing
/// from it, and the next record written from that reading would take the line off the
/// disk as well.
///
/// Credentials are opened here, with the key kept beside the file, because the
/// record is sealed on disk and clear in memory — see [`crate::journal::sealing`].
/// A value this machine has no key for is left sealed rather than dropped: which
/// setting changed is still worth reading where what it changed to is not, and
/// every reader that could act on the value asks whether it opened first.
///
/// # Errors
///
/// [`store::Failure::Unreadable`] where the file is there and cannot be read, or
/// holds a line before its last that is not a change.
pub fn journal_at(path: &Path) -> Result<Journal, store::Failure> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(store::Failure::Unreadable {
                path: path.to_path_buf(),
                reason: error.to_string(),
            })
        }
    };
    let seal = Seal::kept(path);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let last = lines.len().saturating_sub(1);
    let mut changes = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        match serde_json::from_str::<Change>(line) {
            Ok(change) => changes.push(seal.opening(&change)),
            Err(_) if at == last => {}
            Err(why) => {
                return Err(store::Failure::Unreadable {
                    path: path.to_path_buf(),
                    reason: format!(
                        "entry {} is not a change this build can read ({why}), so the journal \
                         is left as it is rather than read without it",
                        at + 1
                    ),
                })
            }
        }
    }
    Ok(Journal::replay(changes))
}

/// Add what a change made to the journal a reversal reads, keeping what is already there
/// and dropping what has fallen outside the bound.
///
/// What is already there is read back and written out again beneath the new entries,
/// because the journal is shared: the first-run wizard wrote what it applied, seeding
/// wrote what it wired, and a repair adding its own must not take either away. The read
/// is [`journal_at`]'s, so a journal it refuses is not written over: the new entries are
/// refused with it, and the file is left for somebody to look at.
///
/// The bound is applied on the way out, so the file itself stays inside it rather than
/// only the reading of it: a record trimmed on read would go on growing on disk, and the
/// horizon would be a claim about what is shown instead of about what is kept.
///
/// Written through the same seam every other record lemonfiber keeps goes through, so the
/// journal is created private to its owner and replaced whole. It holds what a value was
/// before it changed, and a record of an operator's configuration is not something to
/// leave world-readable because this one caller wrote it a different way.
///
/// `random` is what a credential is sealed under: the key where this machine has yet to
/// make one, and a fresh nonce for every value. Every change goes out through the seal,
/// the ones read back included — which is what takes the clear values out of a journal an
/// older version wrote, on the first change recorded after the upgrade.
///
/// # Errors
///
/// Where the journal there cannot be read, or the journal cannot be written. What the
/// caller does about it depends on whether its change is made yet: one recorded before
/// it acts does not act, and one recorded after says the change stands unrecorded
/// ([`unrecorded`]).
pub fn journalled(
    path: &Path,
    changes: &[Change],
    random: &dyn Random,
) -> Result<(), store::Failure> {
    if changes.is_empty() {
        return Ok(());
    }
    let mut held: Vec<Change> = journal_at(path)?.changes().to_vec();
    held.extend(changes.iter().cloned());
    let seal = Seal::minted(path, random);
    let written = kept(&held)
        .iter()
        .filter_map(|change| serde_json::to_string(&seal.sealing(change, random)).ok())
        .fold(String::new(), |mut lines, line| {
            lines.push_str(&line);
            lines.push('\n');
            lines
        });
    store::write(path, &written)
}

/// What a change that was made and could not be recorded amounts to.
///
/// The change stands: it was carried out before the record of it was written, and
/// saying it failed would send an operator to make it again. What it cannot be is put
/// back, because a reversal reads the journal and the journal does not have it — so
/// that is what is said, with the journal's own reason beneath it.
#[must_use]
pub fn unrecorded(what: &str, failure: &store::Failure) -> Problem {
    Problem::new(
        crate::error::codes::undo::CANNOT_SUCCEED,
        Severity::Error,
        format!("{what} was done and could not be recorded, so it cannot be put back"),
        "The change stands. A reversal reads the change journal, and the journal does not \
         have this change in it.",
        Remedy::new("Check that lemonfiber's configuration directory can be written and has space"),
    )
    .caused_by(failure.problem())
}
