//! Telling the stack's own directories from whatever else is in the data location.
//!
//! An operator points the stack at a directory. What they almost never do is point it
//! at an *empty* one — a data location is usually a drive that already had things on
//! it, and the years of photographs in the folder beside the films are exactly what a
//! blanket removal would take.
//!
//! The stack's own tree is small and written down: `downloads`, and one directory
//! under `media` for each media type the services declare. Everything else beneath
//! the data location was put there by somebody, and that is the finding — not a
//! warning to be read past, but the thing that stops the data location from being
//! removed as one tree.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Something beneath the data location that the stack did not put there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Foreign {
    /// The directory it is in, relative to the data location — or the file itself,
    /// where it sits directly in the data location.
    pub at: String,
    /// How many files were found under it.
    pub files: u64,
    /// What they occupy.
    pub bytes: u64,
}

/// The directory the download clients write into, beneath the data location.
const DOWNLOADS: &str = "downloads";

/// The directory the media libraries sit under, beneath the data location.
const MEDIA: &str = "media";

/// The stack's own directories beneath a data location, relative to it.
///
/// `downloads`, and `media/<type>` for each media type the services declare — the
/// same convention the seeding writes root folders against, so a directory this
/// treats as the stack's is one a service was actually pointed at.
#[must_use]
pub fn ours(media_types: &[String]) -> Vec<String> {
    let mut named: Vec<String> = std::iter::once(DOWNLOADS.to_owned())
        .chain(
            media_types
                .iter()
                .map(|media| format!("{MEDIA}/{media}"))
                .collect::<Vec<String>>(),
        )
        .collect();
    named.sort();
    named.dedup();
    named
}

/// Whether a directory is one the stack's own sit *under* — `media`, which holds the
/// libraries, rather than a library itself.
///
/// Descended into rather than reported, because the finding worth making is the
/// directory beside the libraries and not the parent they share with it.
fn leads_to_ours(relative: &Path, ours: &[String]) -> bool {
    let named = relative.to_string_lossy();
    ours.iter()
        .any(|directory| directory.starts_with(&format!("{named}/")))
}

/// What a path beneath the data location is credited to, relative to it.
///
/// The topmost directory that is neither the stack's nor a parent of one, so a
/// photograph library of nine thousand files reads as one finding rather than nine
/// thousand. A file sitting directly in the data location is credited to itself,
/// because there is no directory to name it by.
fn credited(relative: &Path, ours: &[String]) -> String {
    let mut walked = std::path::PathBuf::new();
    for name in relative {
        walked.push(name);
        if !leads_to_ours(&walked, ours) && walked != relative {
            return walked.to_string_lossy().into_owned();
        }
    }
    relative.to_string_lossy().into_owned()
}

/// Everything beneath the data location that the stack did not put there, gathered
/// by the topmost directory that is not one of ours.
///
/// A file the walk reported from outside the data location is not beneath it and is
/// not counted — a walk is asked about a root, and anything else in the answer is
/// something the caller did not ask about.
#[must_use]
pub fn beside(
    root: &Path,
    walked: &[crate::ports::occupancy::Occupant],
    media_types: &[String],
) -> Vec<Foreign> {
    let mut folded = Walked::beneath(root, media_types);
    for occupant in walked {
        folded.add(occupant);
    }
    folded.foreign()
}

/// A walk of the data location, folded as it arrives into what an uninstall reads
/// of it: what it all occupies, what each of the stack's own directories occupies,
/// and what is there that the stack did not put there.
#[derive(Debug, Clone, Default)]
pub struct Walked {
    /// The data location.
    root: PathBuf,
    /// The stack's own directories beneath it.
    ours: Vec<String>,
    /// What every file walked occupies.
    total: u64,
    /// What each of the stack's own directories occupies.
    owned: BTreeMap<String, u64>,
    /// What is there that is not the stack's, by what it is credited to: how many
    /// files, and what they occupy.
    found: BTreeMap<String, (u64, u64)>,
}

impl Walked {
    /// A walk beneath `root`, of a stack whose services hold these media types.
    #[must_use]
    pub fn beneath(root: &Path, media_types: &[String]) -> Self {
        Self {
            root: root.to_path_buf(),
            ours: ours(media_types),
            ..Self::default()
        }
    }

    /// Fold one walked file in.
    pub fn add(&mut self, occupant: &crate::ports::occupancy::Occupant) {
        self.total = self.total.saturating_add(occupant.bytes);
        let Ok(relative) = occupant.path.strip_prefix(&self.root) else {
            return;
        };
        if let Some(directory) = self
            .ours
            .iter()
            .find(|directory| relative.starts_with(directory))
        {
            let held = self.owned.entry(directory.clone()).or_default();
            *held = held.saturating_add(occupant.bytes);
            return;
        }
        let entry = self
            .found
            .entry(credited(relative, &self.ours))
            .or_insert((0, 0));
        entry.0 = entry.0.saturating_add(1);
        entry.1 = entry.1.saturating_add(occupant.bytes);
    }

    /// What every file walked occupies.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.total
    }

    /// What one of the stack's own directories occupies.
    #[must_use]
    pub fn owned(&self, directory: &str) -> u64 {
        self.owned.get(directory).copied().unwrap_or_default()
    }

    /// Everything beneath the data location that the stack did not put there.
    #[must_use]
    pub fn foreign(&self) -> Vec<Foreign> {
        self.found
            .iter()
            .map(|(at, (files, bytes))| Foreign {
                at: at.clone(),
                files: *files,
                bytes: *bytes,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
