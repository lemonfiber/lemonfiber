//! Where the disk went, when it runs out, and what can be got back.
//!
//! A media stack's natural trajectory is to fill whatever disk it is given, and a
//! full disk is the worst common failure because it breaks everything at once —
//! downloads stall, imports fail, and the services' own databases stop being able
//! to write, which is how a space problem becomes a data-loss problem.
//!
//! Three decisions shape everything here.
//!
//! **Exhaustion is projected rather than reported.** Free space is a lagging
//! indicator; what matters is free space against what is already committed to
//! landing on it. A warning that arrives when the disk is full is a description.
//!
//! **Nothing is ever deleted unasked.** What is reclaimable is identified and
//! offered, and taking it needs an answer. The operator may have something
//! irreplaceable and no heuristic is worth that risk, so the rule is absolute
//! rather than threshold-dependent: there is no level at which this product
//! removes media because it decided to.
//!
//! **The accounting counts underlying files, not names.** With hardlinks working —
//! which is the arrangement this whole product exists to keep working — a file in
//! the downloads tree and in the library is one file. Summing the two listings
//! reports twice what is there, and every figure built on that sum is wrong by the
//! same amount, including which cleanup looks worth doing.
//!
//! The two readings are both kept. What the tree would take on a filesystem that
//! could not link is what somebody is quoted when they think about moving it; what
//! it occupies is what the volume has lost.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

pub mod category;
pub mod letting;
pub mod level;
pub mod outsized;
pub(crate) mod run;
pub mod tally;
pub mod unpacked;
pub mod volume;
pub mod waste;

pub use category::{Category, Consumption, Reclaim};
pub use letting::{Gone, Letting, WHAT_GOES};
pub use level::Level;
pub use outsized::Outsized;
pub use tally::{Counting, Tally};
pub use volume::{Freshness, Role, Volume};
pub use waste::{ratio_reads, Candidate, Standing, RATIO_CONSEQUENCE};

use crate::ports::occupancy::Occupant;
use crate::ports::service::Seeded;

pub(crate) use crate::error::codes::space::HALTED;

pub(crate) use crate::error::codes::space::NOWHERE_TO_MEASURE;

pub(crate) use crate::error::codes::space::WALK_REFUSED;

pub(crate) use crate::error::codes::space::NOTHING_TO_ASK;

pub(crate) use crate::error::codes::space::NOT_HELD;

pub(crate) use crate::error::codes::space::ANOTHER_OFFER;

pub use crate::error::codes::space::STILL_HELD;

/// An import that stopped part-way, in the words of whatever stopped it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Interrupted {
    /// What the service calls it.
    pub name: String,
    /// What is on disk for it already, where the walk could find it.
    pub partial: u64,
    /// What the service said, verbatim.
    pub said: String,
}

/// An import the service has stopped making progress on, as one read found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stalled {
    /// What the service calls it.
    pub name: String,
    /// What the service said about why, where it said anything.
    pub said: Option<String>,
}

/// What became of a confirmed cleanup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Reclaimed {
    /// The paths that were taken.
    pub gone: Vec<String>,
    /// What they occupied.
    pub bytes: u64,
    /// What could not be taken, and what the platform said about it.
    pub left: Vec<Left>,
}

/// Something a cleanup could not take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "SpaceLeft")]
pub struct Left {
    /// Where it is.
    pub at: String,
    /// What the platform said, verbatim.
    pub why: String,
}

/// Everything one reckoning is made from, gathered before any of it is judged.
///
/// A struct rather than a dozen arguments, and gathered whole before anything is
/// decided, so that every figure in one report describes one moment. Two readings
/// taken a second apart can disagree about a disk that filled between them, and a
/// report assembled from both would be internally inconsistent in a way nobody
/// could see.
#[derive(Debug, Default)]
pub struct Measured {
    /// The volumes watched, in the order they are reported.
    pub volumes: Vec<Volume>,
    /// The data location, which the walk below is relative to.
    pub root: PathBuf,
    /// Every file beneath the data location.
    pub data: Vec<Occupant>,
    /// Every file the services keep of their own.
    pub services: Vec<Occupant>,
    /// What the download clients still have to write.
    pub landing: u64,
    /// The completed downloads the clients are still holding.
    pub held: Vec<Seeded>,
    /// What the services still have in their queues, by the name both sides use.
    pub awaited: BTreeSet<String>,
    /// The imports that have stopped making progress.
    pub stalled: Vec<Stalled>,
    /// What the operator asked to be left alone, by the same name.
    pub marked: BTreeSet<String>,
}

/// Where the disk stands, what is on it, and what could be got back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Reckoning {
    /// The volumes watched. Either filling stops the stack, so both are reported
    /// whether or not they are the same drive.
    pub volumes: Vec<Volume>,
    /// Where the stack stands, which is where its worst volume stands.
    pub level: Level,
    /// Whether new acquisitions are halted to keep the services writable.
    pub halted: bool,
    /// Where the room went, one line per tree plus the services' own files, and
    /// one line for what is committed but has not landed yet.
    pub consumption: Vec<Consumption>,
    /// What of that room could be got back, and what each would cost.
    ///
    /// A second reading of bytes already counted above rather than more of them: a
    /// seeding torrent's file is in the tree it lives in *and* here. Summing the
    /// two lists together would double what is on the disk, which is the mistake
    /// this whole module is arranged to avoid.
    pub reclaimable: Vec<Consumption>,
    /// The completed downloads, each with where it stands and what removing it
    /// would cost.
    pub candidates: Vec<Candidate>,
    /// The files far enough out of line with the rest to be worth pointing at.
    pub outsized: Vec<Outsized>,
    /// The imports that stopped part-way, with what is on disk for each.
    pub interrupted: Vec<Interrupted>,
    /// What this offer names itself, so an answer to it can say which offer it was
    /// answering.
    pub agreement: String,
    /// What became of a confirmed cleanup, where one was asked for.
    pub reclaimed: Option<Reclaimed>,
}

impl Reckoning {
    /// The paths a confirmed cleanup would take.
    ///
    /// Only what costs nothing: the downloads nothing ever imported, and the
    /// archive parts whose contents are already unpacked beside them. Everything
    /// else reclaimable is named in the report and left with the operator, because
    /// what it costs is not this product's to weigh.
    #[must_use]
    pub fn offering<'a>(&self, measured: &'a Measured) -> Vec<&'a Occupant> {
        let taking: BTreeSet<&str> = self
            .candidates
            .iter()
            .filter(|candidate| candidate.offered())
            .map(|candidate| candidate.name.as_str())
            .collect();
        let mut offered: Vec<&Occupant> = measured
            .data
            .iter()
            .filter(|occupant| belongs_to_any(&occupant.path, &taking))
            .collect();
        offered.extend(unpacked::already_unpacked(&measured.data));
        offered.sort_by(|left, right| left.path.cmp(&right.path));
        offered.dedup_by(|left, right| left.path == right.path);
        offered
    }
}

/// Whether a walked file belongs to any of these named downloads.
fn belongs_to_any(path: &Path, names: &BTreeSet<&str>) -> bool {
    names.iter().any(|name| {
        let wanted = std::ffi::OsStr::new(name);
        path.components().any(|part| part.as_os_str() == wanted)
            || path.file_stem().is_some_and(|stem| stem == wanted)
    })
}

/// Judge what was measured.
///
/// Everything here is decided from the values handed in and nothing is kept
/// between runs, which is what makes space freed outside this product clear the
/// condition: the next reckoning measures the disk as it now is and has no earlier
/// verdict to carry forward.
#[must_use]
pub fn reckon(measured: &Measured) -> Reckoning {
    let candidates = waste::candidates(
        &measured.held,
        &measured.awaited,
        &measured.marked,
        &measured.data,
    );
    let level = Level::worst(measured.volumes.iter().map(|volume| volume.level));
    let agreement = naming(&candidates);
    Reckoning {
        volumes: measured.volumes.clone(),
        halted: level.halts(),
        level,
        consumption: consumption(measured),
        reclaimable: reclaimable(measured, &candidates),
        outsized: outsized::outsized(&measured.data),
        interrupted: interrupted(measured, &candidates),
        candidates,
        agreement,
        reclaimed: None,
    }
}

/// Where the room went.
///
/// The trees are counted through one running count rather than each on its own,
/// so a file the downloads tree and the library both hold is charged once across
/// the whole report — which is the difference between a total that matches the
/// volume and one that is twice it.
fn consumption(measured: &Measured) -> Vec<Consumption> {
    let mut counting = Counting::default();
    let mut lines: Vec<Consumption> = trees(&measured.root, &measured.data)
        .into_iter()
        .map(|(name, files)| Consumption::of(Category::Tree(name), counting.count(&files)))
        .collect();
    lines.push(Consumption::of(
        Category::Services,
        Counting::default().count(&measured.services),
    ));
    lines.push(Consumption::of(
        Category::Landing,
        Tally {
            logical: measured.landing,
            physical: measured.landing,
            files: 0,
            shared: 0,
        },
    ));
    lines.retain(Consumption::any);
    lines
}

/// What of that room could be got back.
fn reclaimable(measured: &Measured, candidates: &[Candidate]) -> Vec<Consumption> {
    let mut lines = vec![
        Consumption::of(
            Category::Orphaned,
            standing_tally(measured, candidates, |standing| {
                matches!(standing, Standing::NeverImported)
            }),
        ),
        Consumption::of(
            Category::Seeding,
            standing_tally(measured, candidates, |standing| {
                matches!(standing, Standing::Seeding { .. })
            }),
        ),
        Consumption::of(
            Category::Extracted,
            Counting::default().count(
                &unpacked::already_unpacked(&measured.data)
                    .into_iter()
                    .cloned()
                    .collect::<Vec<Occupant>>(),
            ),
        ),
        Consumption::of(
            Category::Unmanaged,
            standing_tally(measured, candidates, |standing| {
                matches!(standing, Standing::LeftAlone)
            }),
        ),
    ];
    lines.retain(Consumption::any);
    lines
}

/// What the downloads standing one particular way occupy, counted off the walk.
///
/// Off the walk rather than off the client's own byte counts, because the client
/// reports what the torrent is and the walk reports what the disk holds — and on a
/// linked stack those differ by exactly the thing this module exists to get right.
fn standing_tally(
    measured: &Measured,
    candidates: &[Candidate],
    wanted: fn(Standing) -> bool,
) -> Tally {
    let names: BTreeSet<&str> = candidates
        .iter()
        .filter(|candidate| wanted(candidate.standing))
        .map(|candidate| candidate.name.as_str())
        .collect();
    let files: Vec<Occupant> = measured
        .data
        .iter()
        .filter(|occupant| belongs_to_any(&occupant.path, &names))
        .cloned()
        .collect();
    Counting::default().count(&files)
}

/// The imports that stopped part-way, with what is on disk for each.
///
/// Reported whatever the level, because an import that stopped is worth knowing
/// about either way — and reported *with* the cleanup above it when the disk is
/// what stopped it, so the retry has somewhere to go.
fn interrupted(measured: &Measured, candidates: &[Candidate]) -> Vec<Interrupted> {
    measured
        .stalled
        .iter()
        .map(|stalled| Interrupted {
            partial: candidates
                .iter()
                .find(|candidate| candidate.name == stalled.name)
                .map_or(0, |candidate| candidate.bytes),
            name: stalled.name.clone(),
            said: stalled
                .said
                .clone()
                .unwrap_or_else(|| "the service gave no reason".to_owned()),
        })
        .collect()
}

/// One entry per directory directly beneath the root, holding the files under it.
///
/// Per directory rather than one figure for everything, because several libraries
/// commonly share a volume and a single total says nothing about which of them is
/// growing. A file sitting directly in the root, under no directory at all, is
/// grouped under the root's own name so that nothing walked goes unaccounted for.
fn trees(root: &Path, occupants: &[Occupant]) -> Vec<(String, Vec<Occupant>)> {
    let mut grouped: BTreeMap<String, Vec<Occupant>> = BTreeMap::new();
    for occupant in occupants {
        grouped
            .entry(tree_of(root, &occupant.path))
            .or_default()
            .push(occupant.clone());
    }
    grouped.into_iter().collect()
}

/// Which tree a walked file belongs to.
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

/// What this offer names itself.
///
/// Built from what would actually be taken and what each of them is, so an answer
/// given against one listing cannot be spent on a different one: a download that
/// has finished seeding since the offer was read makes this a different name, and
/// the answer is refused rather than acting on something nobody saw.
fn naming(candidates: &[Candidate]) -> String {
    let words: Vec<String> = candidates
        .iter()
        .filter(|candidate| candidate.offered())
        .map(|candidate| format!("{}:{}", candidate.name, candidate.bytes))
        .collect();
    crate::agreement::over(&words.iter().map(String::as_str).collect::<Vec<&str>>())
}

#[cfg(test)]
mod tests;
