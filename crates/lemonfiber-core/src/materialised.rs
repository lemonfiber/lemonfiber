//! The record of what lemonfiber last wrote to the stack directory.
//!
//! The stack ships inside the binary and is written to disk so Compose can read
//! it. Those files are the operator's to edit, so re-writing them on a later run
//! must not clobber a change they made by hand. To tell an edit from a version the
//! operator has not upgraded yet, lemonfiber has to remember what it last wrote —
//! this is that memory, a checksum per file, the same expected/actual/desired
//! comparison the seed baseline makes, over file content rather than a setting.
//!
//! A checksum, not the content: a change is what matters, not what it was, and a
//! crc is enough to notice an accidental edit — this is not a defence against a
//! crafted collision, which no upgrade policy needs.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// A checksum of every stack file lemonfiber last wrote, keyed by the file's path
/// within the stack directory. Ordered, so the file it serialises to is stable from
/// one run to the next rather than reshuffled on every write.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Materialised {
    files: BTreeMap<String, u32>,
}

impl Materialised {
    /// An empty record — nothing materialised yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the checksum of a file lemonfiber wrote, as the expected state a later
    /// run compares the file on disk against.
    pub fn record(&mut self, path: &str, checksum: u32) {
        self.files.insert(path.to_owned(), checksum);
    }

    /// The checksum lemonfiber last wrote for a file, or `None` where it wrote none —
    /// which a later run reads as "never written by lemonfiber", so a file on disk
    /// there is the operator's alone, not a difference from anything lemonfiber wrote.
    #[must_use]
    pub fn checksum(&self, path: &str) -> Option<u32> {
        self.files.get(path).copied()
    }
}

/// The checksum of a file's content.
#[must_use]
pub fn checksum(content: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(content);
    hasher.finalize()
}

/// What to do with one stack file, from the three-way comparison of what lemonfiber
/// last wrote (`expected`), what is on disk now (`actual`), and what it would write
/// (`desired`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Absent from disk, or unchanged since lemonfiber wrote it: write what lemonfiber
    /// has. This is the ordinary path — a first materialise, or an upgrade of a file
    /// the operator never touched.
    Write,
    /// Already exactly what lemonfiber would write: nothing to do, so a re-run leaves
    /// the file — and its timestamp — untouched.
    Fresh,
    /// Changed from what lemonfiber last wrote: the operator's edit. It is preserved,
    /// never overwritten on an upgrade without their say-so.
    Preserve,
}

/// Decide what to do with one file from the three-way comparison.
///
/// A file that is not on disk is written. One already at what lemonfiber would write
/// is left. Otherwise the difference is either lemonfiber's own — the file still
/// holds what it last wrote, while its embedded content has moved on to an upgrade,
/// which is written — or the operator's, where the file no longer holds what
/// lemonfiber wrote, which is preserved. With no record to judge against, a file
/// that differs is taken as the operator's and preserved rather than overwritten on
/// a guess.
#[must_use]
pub fn decide(expected: Option<u32>, actual: Option<u32>, desired: u32) -> Decision {
    match actual {
        None => Decision::Write,
        Some(actual) if actual == desired => Decision::Fresh,
        Some(actual) => match expected {
            Some(expected) if actual == expected => Decision::Write,
            _ => Decision::Preserve,
        },
    }
}

/// A line diff of an operator's edited file against what lemonfiber would write,
/// so the operator is shown what an upgrade would change rather than only that it
/// would. The lines only in their file are marked `-`, the lines only in
/// lemonfiber's `+`; the matching head and tail are left out, so what is shown is
/// the change and not the whole file.
///
/// A line that carries a credential is shown by name with its value withheld — on
/// **both** sides, so a drifted secret is reported without displaying either the
/// operator's value or lemonfiber's. A diff is printed to a terminal, read into
/// scrollback and pasted into bug reports, and a key that reaches any of those is a
/// key that has to be rotated.
#[must_use]
pub fn diff(yours: &str, ours: &str) -> String {
    let yours: Vec<&str> = yours.lines().collect();
    let ours: Vec<&str> = ours.lines().collect();
    // The matching head, then the matching tail that does not run back into it.
    let head = yours.iter().zip(&ours).take_while(|(a, b)| a == b).count();
    let tail = yours
        .iter()
        .rev()
        .zip(ours.iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(yours.len() - head)
        .min(ours.len() - head);
    let your_middle = yours.get(head..yours.len() - tail).unwrap_or_default();
    let our_middle = ours.get(head..ours.len() - tail).unwrap_or_default();
    let mut out = String::new();
    for line in your_middle {
        let _ = writeln!(out, "- {}", shown(line));
    }
    for line in our_middle {
        let _ = writeln!(out, "+ {}", shown(line));
    }
    out
}

/// One line of a stack file as it is safe to show.
///
/// Through the allow-list, which is the same list `/api/config` reads the operator's
/// settings against — so a value withheld there is withheld here, and a run cannot
/// print in its diff what it withheld in its listing. A marker list cannot do that: it
/// knows `APIKEY` and not `OPENVPN_USER`, where a provider's account number is half of
/// a paid login.
fn shown(line: &str) -> String {
    crate::config::store::withheld_by(line, &crate::config::display::shown_in_a_file)
}

#[cfg(test)]
mod tests;
