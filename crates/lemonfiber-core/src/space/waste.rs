//! Telling a download nothing ever took from one that is seeding after a good
//! import.
//!
//! From the download client alone the two are identical: both are complete, both
//! are sitting on disk, and no service is waiting for either. Guessing between
//! them is the mistake this product must not make in either direction — call every
//! healthy seed an orphan and the operator turns the report off; call every orphan
//! a seed and the easy win nobody knew about stays where it is.
//!
//! What tells them apart is the filesystem, and it has been telling anyone who
//! asked all along. An import hardlinks: after it, the file has a second name in
//! the library and the count of names is two. A download nothing ever took has
//! exactly one name, because nothing ever made a second. So the count of names is
//! the evidence, and it is evidence rather than inference.
//!
//! Nothing is ever called waste on an absence. A download with no file this walk
//! could match is not reported at all, because "I could not find it" and "nothing
//! points at it" would then read the same — and one of them is a reason to delete
//! something.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::ports::occupancy::Occupant;
use crate::ports::service::Seeded;

/// What removing a seeding torrent costs, said the same way wherever it is said.
///
/// Stated in what it does rather than in what it is: "affects your ratio" means
/// nothing to somebody who has not been thrown off a tracker for it, and this is
/// what it means.
pub const RATIO_CONSEQUENCE: &str =
    "Removing it stops it seeding. On a private tracker the ratio it is still \
     earning is what your account is kept on, and losing it can cost the account \
     rather than the file.";

/// Where one completed download stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "standing")]
#[schemars(rename = "SeedingStanding")]
pub enum Standing {
    /// Nothing ever linked it into a library: it was never imported, and removing
    /// it loses nothing.
    NeverImported,
    /// It was imported and is still seeding, so removing it has a consequence
    /// outside this machine.
    Seeding {
        /// What it has uploaded against what it downloaded, in hundredths, as the
        /// client reports it.
        ///
        /// A whole number rather than a fraction because every report this product
        /// makes is compared for equality somewhere, and a fraction cannot be —
        /// two figures a client would call the same would not be. The hundredth is
        /// finer than any decision made on a ratio.
        ratio: u32,
    },
    /// The operator asked for this one to be left alone.
    LeftAlone,
}

/// One completed download, and what reclaiming it would come to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Candidate {
    /// What both sides call it.
    pub name: String,
    /// What it occupies.
    pub bytes: u64,
    /// Where it stands.
    pub standing: Standing,
    /// What removing it costs, where it costs anything.
    pub consequence: Option<String>,
}

impl Candidate {
    /// Whether lemonfiber will offer to remove this one.
    ///
    /// Only the ones that cost nothing. A seeding torrent is named, sized and
    /// explained, and then left with the operator — it is not that the cost is too
    /// large to accept, it is that nothing here can weigh it.
    #[must_use]
    pub const fn offered(&self) -> bool {
        matches!(self.standing, Standing::NeverImported)
    }
}

/// Which completed downloads are on disk, and what each one is.
///
/// `awaited` is what the services still have in their queues, `marked` is what the
/// operator has asked to be left alone, and `occupants` is the walk of the data
/// root. A download the walk could not match to any file is left out entirely.
#[must_use]
pub fn candidates(
    held: &[Seeded],
    awaited: &BTreeSet<String>,
    marked: &BTreeSet<String>,
    occupants: &[Occupant],
) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = held
        .iter()
        .filter_map(|download| {
            let files = belonging(occupants, &download.name);
            if files.is_empty() {
                return None;
            }
            let standing = standing(download, awaited, marked, &files);
            Some(Candidate {
                consequence: consequence(standing),
                standing,
                name: download.name.clone(),
                bytes: download.bytes,
            })
        })
        .collect();
    // Largest first, then by name, so the line worth acting on is at the top and
    // two runs over one stack read alike.
    found.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.name.cmp(&right.name))
    });
    found
}

/// Where one download stands, given the files that belong to it.
fn standing(
    download: &Seeded,
    awaited: &BTreeSet<String>,
    marked: &BTreeSet<String>,
    files: &[&Occupant],
) -> Standing {
    if marked.contains(&download.name) {
        return Standing::LeftAlone;
    }
    let linked = files
        .iter()
        .any(|occupant| occupant.identity.is_some_and(|identity| identity.links > 1));
    if linked || awaited.contains(&download.name) {
        return Standing::Seeding {
            ratio: download.ratio,
        };
    }
    Standing::NeverImported
}

/// What removing this one costs, where it costs anything.
///
/// Read off where it stands rather than taken beside it, so a download can never
/// be reported as costing nothing while standing somewhere that costs something.
fn consequence(standing: Standing) -> Option<String> {
    match standing {
        Standing::NeverImported => None,
        Standing::Seeding { .. } => Some(RATIO_CONSEQUENCE.to_owned()),
        Standing::LeftAlone => Some(
            "You asked for this one to be left alone, so nothing here will take it.".to_owned(),
        ),
    }
}

/// A seeding ratio as a person reads it, from the hundredths the client reports.
///
/// Nothing where there is no ratio to read: a torrent added from files already on
/// disk downloaded nothing, so what it has given back is not divisible by what it
/// took, and the largest figure this can carry stands for that. Printing that
/// figure would show somebody forty-two million, which is a number rather than an
/// answer — so the caller says the thing in words instead.
#[must_use]
pub fn ratio_reads(hundredths: u32) -> Option<String> {
    (hundredths != u32::MAX).then(|| format!("{}.{:02}", hundredths / 100, hundredths % 100))
}

/// The walked files that belong to a named download.
///
/// Matched by name, which is what both sides call it — the same correlation the
/// queue check makes between a client and a service. A download's name is either a
/// directory it was written into or the file itself, so either a path component or
/// a file stem matching is enough.
fn belonging<'a>(occupants: &'a [Occupant], name: &str) -> Vec<&'a Occupant> {
    occupants
        .iter()
        .filter(|occupant| under(&occupant.path, name))
        .collect()
}

/// Whether a path is this download's: a directory of that name above it, or the
/// file itself named that.
fn under(path: &Path, name: &str) -> bool {
    let wanted = std::ffi::OsStr::new(name);
    path.components().any(|part| part.as_os_str() == wanted)
        || path.file_stem().is_some_and(|stem| stem == wanted)
}

#[cfg(test)]
mod tests;
