//! What a walk of the data location is kept as: only what each reading of it needs.
//!
//! A library is hundreds of thousands of files, and holding every one of them —
//! its path, its size, which file it is — while the reckoning is made is tens of
//! megabytes held for the length of a request. What the reckoning reads from them
//! is much less than that, so each file is folded in as it arrives and only these
//! are kept:
//!
//! - **Where the room went:** a count per directory beneath the root, with the
//!   identity of each file reachable under more than one name so it is charged once.
//! - **What is out of line:** every file's size, for the middle one, and the few
//!   largest files that are big enough to be worth naming.
//! - **What a cleanup would take:** every file belonging to a download the client is
//!   holding, and every archive part with one file beside it that is not an archive.
//!   These are what an answered cleanup removes, so they are kept whole.
//!
//! Nothing here depends on the order files arrive in. A shared file is charged to
//! the directory that comes first by name, which is the order the directories are
//! reported in, whichever of its names the walk happened to reach first.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::ports::occupancy::Occupant;
use crate::space::outsized::{FLOOR, MOST};
use crate::space::unpacked::is_archive;
use crate::space::Tally;

/// One directory's count, with what is needed to charge a shared file once.
#[derive(Debug, Default)]
struct Tree {
    /// The bytes every name adds up to.
    logical: u64,
    /// How many names were walked.
    files: usize,
    /// The bytes of the files nothing else names.
    alone: u64,
    /// Each file reachable under more than one name: its size, and how many of those
    /// names this directory holds.
    linked: BTreeMap<u64, (u64, usize)>,
}

/// A walk of the data location, folded as it arrives.
#[derive(Debug, Default)]
pub struct Survey {
    /// The data location the walk was made beneath.
    root: PathBuf,
    /// The downloads the client is holding, by the name both sides call them.
    held: BTreeSet<String>,
    /// Each directory beneath the root, by name.
    trees: BTreeMap<String, Tree>,
    /// Every file's size.
    sizes: Vec<u64>,
    /// The largest files at or above [`FLOOR`], largest first, at most [`MOST`].
    largest: Vec<Occupant>,
    /// Every file belonging to a download the client is holding.
    holding: Vec<Occupant>,
    /// Every archive part, by the directory it is in.
    archives: BTreeMap<PathBuf, Vec<Occupant>>,
    /// One file that is not an archive, for each directory that holds one.
    beside: BTreeMap<PathBuf, Occupant>,
}

impl Survey {
    /// A survey of the walk beneath `root`, keeping the files of the downloads the
    /// client is holding by these names.
    #[must_use]
    pub fn beneath(root: &Path, held: impl IntoIterator<Item = String>) -> Self {
        Self {
            root: root.to_path_buf(),
            held: held.into_iter().collect(),
            trees: BTreeMap::new(),
            sizes: Vec::new(),
            largest: Vec::new(),
            holding: Vec::new(),
            archives: BTreeMap::new(),
            beside: BTreeMap::new(),
        }
    }

    /// Fold one walked file in.
    pub fn add(&mut self, occupant: Occupant) {
        let tree = self
            .trees
            .entry(tree_of(&self.root, &occupant.path))
            .or_default();
        tree.logical = tree.logical.saturating_add(occupant.bytes);
        tree.files += 1;
        match occupant
            .identity
            .filter(|identity| identity.links > 1 && identity.file != 0)
        {
            Some(identity) => {
                let (_, names) = tree
                    .linked
                    .entry(identity.file)
                    .or_insert((occupant.bytes, 0));
                *names += 1;
            }
            None => tree.alone = tree.alone.saturating_add(occupant.bytes),
        }
        self.sizes.push(occupant.bytes);
        if occupant.bytes >= FLOOR {
            self.largest.push(occupant.clone());
            self.largest.sort_by(|left, right| {
                right
                    .bytes
                    .cmp(&left.bytes)
                    .then_with(|| left.path.cmp(&right.path))
            });
            self.largest.truncate(MOST);
        }
        if self.is_held(&occupant.path) {
            self.holding.push(occupant.clone());
        }
        let folder = occupant
            .path
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        if is_archive(&occupant.path) {
            self.archives.entry(folder).or_default().push(occupant);
        } else {
            self.beside.entry(folder).or_insert(occupant);
        }
    }

    /// Whether a walked file belongs to any download the client is holding: a
    /// directory above it, or the file itself, named as one of them.
    ///
    /// Each part of the path looked up among the names, rather than each name tried
    /// against the path, because a library is walked once and the client may hold
    /// hundreds of downloads.
    fn is_held(&self, path: &Path) -> bool {
        let named =
            |part: &std::ffi::OsStr| part.to_str().is_some_and(|part| self.held.contains(part));
        path.components().any(|part| named(part.as_os_str())) || path.file_stem().is_some_and(named)
    }

    /// Each directory beneath the root and what it occupies, in name order, a file
    /// reachable from two of them charged to the first.
    #[must_use]
    pub fn trees(&self) -> Vec<(String, Tally)> {
        let mut seen = BTreeSet::new();
        self.trees
            .iter()
            .map(|(name, tree)| {
                let mut tally = Tally {
                    logical: tree.logical,
                    physical: tree.alone,
                    files: tree.files,
                    shared: 0,
                };
                for (file, (bytes, names)) in &tree.linked {
                    if seen.insert(*file) {
                        tally.physical = tally.physical.saturating_add(*bytes);
                        tally.shared += names - 1;
                    } else {
                        tally.shared += names;
                    }
                }
                (name.clone(), tally)
            })
            .collect()
    }

    /// The size of the middle file, or nothing where there is nothing to compare
    /// against.
    ///
    /// The middle rather than the mean, because the mean of a library holding one
    /// enormous file is dragged towards that file — the comparison would then be
    /// against the thing being looked for, which is how an outlier hides itself. A
    /// walk whose middle file is empty gives no ratio to compare against, so nothing
    /// is reported rather than everything.
    #[must_use]
    pub fn typical(&self) -> Option<u64> {
        let mut sizes = self.sizes.clone();
        let middle = sizes.len() / 2;
        if middle < sizes.len() {
            sizes.select_nth_unstable(middle);
        }
        sizes.get(middle).copied().filter(|size| *size > 0)
    }

    /// The largest files big enough to be worth naming, largest first.
    #[must_use]
    pub fn largest(&self) -> &[Occupant] {
        &self.largest
    }

    /// Every file belonging to a download the client is holding.
    #[must_use]
    pub fn holding(&self) -> &[Occupant] {
        &self.holding
    }

    /// Every archive part, and one file that is not an archive beside each folder of
    /// them that has one — what tells an archive already unpacked from one that is not.
    #[must_use]
    pub fn unpacking(&self) -> Vec<Occupant> {
        let mut found: Vec<Occupant> = Vec::new();
        for (folder, parts) in &self.archives {
            found.extend(parts.iter().cloned());
            found.extend(self.beside.get(folder).cloned());
        }
        found
    }
}

/// Whether a path is this download's: a directory of that name above it, or the
/// file itself named that.
pub(crate) fn belongs(path: &Path, name: &str) -> bool {
    let wanted = std::ffi::OsStr::new(name);
    path.components().any(|part| part.as_os_str() == wanted)
        || path.file_stem().is_some_and(|stem| stem == wanted)
}

/// Which directory beneath the root a walked file belongs to.
///
/// Named components only, so that a path this walk did not take from beneath the
/// root — which nothing should produce, and which must not be lost if something
/// does — is named by the first directory in it rather than by the separator at
/// the front of it.
fn tree_of(root: &Path, path: &Path) -> String {
    let under = path.strip_prefix(root).unwrap_or(path);
    let mut parts = under
        .components()
        .filter(|part| matches!(part, std::path::Component::Normal(_)));
    match (parts.next(), parts.next()) {
        // A file directly in the root has no directory of its own to be named by.
        (Some(_), None) | (None, _) => "the data location itself".to_owned(),
        (Some(first), Some(_)) => first.as_os_str().to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests;
