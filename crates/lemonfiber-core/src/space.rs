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
pub mod level;
pub mod outsized;
pub mod tally;
pub mod unpacked;
pub mod volume;
pub mod waste;

pub use category::{Category, Consumption, Reclaim};
pub use level::Level;
pub use outsized::Outsized;
pub use tally::{Counting, Tally};
pub use volume::{Freshness, Role, Volume};
pub use waste::{ratio_reads, Candidate, Standing, RATIO_CONSEQUENCE};

use crate::ports::occupancy::Occupant;
use crate::ports::service::Seeded;

/// Raised when the volume is full and new acquisitions are therefore halted.
pub const HALTED: crate::error::Code = crate::error::Code::new("SPACE-1");

/// Raised when there is no data location to measure.
pub const NOWHERE_TO_MEASURE: crate::error::Code = crate::error::Code::new("SPACE-2");

/// Raised when the data location is there and could not be read.
pub const WALK_REFUSED: crate::error::Code = crate::error::Code::new("SPACE-3");

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
    wanted: fn(&Standing) -> bool,
) -> Tally {
    let names: BTreeSet<&str> = candidates
        .iter()
        .filter(|candidate| wanted(&candidate.standing))
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
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::{reckon, Category, Measured, Occupant, Seeded, Stalled, Standing, Volume};
    use crate::ports::filesystem::{FsKind, Identity, StorageFacts};
    use crate::space::Role;

    /// A walked file with a given number of names pointing at it.
    fn file(path: &str, bytes: u64, inode: u64, links: u64) -> Occupant {
        Occupant {
            path: PathBuf::from(path),
            bytes,
            identity: Some(Identity { file: inode, links }),
        }
    }

    /// A volume with room to spare, so a case about the accounting is not also a
    /// case about the level.
    fn roomy(role: Role, at: &str) -> Volume {
        Volume::measured(
            role,
            &PathBuf::from(at),
            &StorageFacts {
                point: PathBuf::from("/srv"),
                kind: FsKind::classify("ext4"),
                removable: false,
                available: 900_000_000_000,
                total: 1_000_000_000_000,
            },
            0,
            0,
        )
    }

    /// A stack whose data root holds one imported file and one that was never
    /// taken, with the client still holding both.
    fn a_stack() -> Measured {
        Measured {
            volumes: vec![roomy(Role::Data, "/srv/media")],
            root: PathBuf::from("/srv/media"),
            data: vec![
                file("/srv/media/downloads/Imported/a.mkv", 8_000, 41, 2),
                file("/srv/media/films/Imported/a.mkv", 8_000, 41, 2),
                file("/srv/media/downloads/Never.Taken/b.mkv", 3_000, 42, 1),
            ],
            services: vec![file("/srv/lemonfiber/config/sonarr.db", 500, 90, 1)],
            landing: 1_000,
            held: vec![
                Seeded {
                    name: "Imported".to_owned(),
                    bytes: 8_000,
                    ratio: 175,
                },
                Seeded {
                    name: "Never.Taken".to_owned(),
                    bytes: 3_000,
                    ratio: 0,
                },
            ],
            awaited: BTreeSet::new(),
            stalled: Vec::new(),
            marked: BTreeSet::new(),
        }
    }

    /// What one category of the report came to, by heading.
    fn line(lines: &[super::Consumption], heading: &str) -> Option<super::Tally> {
        lines
            .iter()
            .find(|line| line.category.heading() == heading)
            .map(|line| line.tally)
    }

    #[test]
    fn a_file_in_two_trees_is_charged_to_the_disk_once() {
        let reckoned = reckon(&a_stack());
        let downloads = line(&reckoned.consumption, "downloads").unwrap_or_default();
        let films = line(&reckoned.consumption, "films").unwrap_or_default();
        assert_eq!(downloads.physical, 11_000, "both downloads, once each");
        assert_eq!(
            films.physical, 0,
            "the library's copy is the same file, already paid for"
        );
        assert_eq!(films.logical, 8_000, "and it is still eight thousand bytes");
        assert!(
            films.differs(),
            "the two readings are worth reporting apart"
        );
    }

    #[test]
    fn every_tree_is_a_line_of_its_own() {
        let reckoned = reckon(&a_stack());
        let headings: Vec<String> = reckoned
            .consumption
            .iter()
            .map(|line| line.category.heading())
            .collect();
        assert!(headings.contains(&"downloads".to_owned()));
        assert!(headings.contains(&"films".to_owned()));
        assert!(headings.contains(&"the services' own files".to_owned()));
        assert!(headings.contains(&"still to land".to_owned()));
    }

    #[test]
    fn what_was_never_imported_is_the_easy_win_and_what_is_seeding_is_not() {
        let reckoned = reckon(&a_stack());
        let orphaned = line(&reckoned.reclaimable, "never imported").unwrap_or_default();
        let seeding = line(&reckoned.reclaimable, "seeding").unwrap_or_default();
        assert_eq!(orphaned.physical, 3_000);
        assert_eq!(seeding.physical, 8_000);

        let offered: Vec<&str> = reckoned
            .candidates
            .iter()
            .filter(|candidate| candidate.offered())
            .map(|candidate| candidate.name.as_str())
            .collect();
        assert_eq!(offered, ["Never.Taken"]);
    }

    #[test]
    fn what_a_confirmed_cleanup_would_take_is_only_what_costs_nothing() {
        let measured = a_stack();
        let taking: Vec<String> = reckon(&measured)
            .offering(&measured)
            .into_iter()
            .map(|occupant| occupant.path.display().to_string())
            .collect();
        assert_eq!(taking, ["/srv/media/downloads/Never.Taken/b.mkv"]);
    }

    #[test]
    fn nothing_is_offered_for_what_the_operator_asked_to_be_left_alone() {
        let mut measured = a_stack();
        measured.marked = BTreeSet::from(["Never.Taken".to_owned()]);
        let reckoned = reckon(&measured);
        assert!(
            reckoned.offering(&measured).is_empty(),
            "the instruction is followed rather than weighed"
        );
        assert!(reckoned
            .candidates
            .iter()
            .any(|candidate| candidate.standing == Standing::LeftAlone));
        assert!(
            line(&reckoned.reclaimable, "left alone at your request").is_some(),
            "and the room it takes is still accounted for"
        );
    }

    #[test]
    fn an_offer_that_has_moved_on_names_itself_differently() {
        let first = reckon(&a_stack());
        let mut later = a_stack();
        later.held.push(Seeded {
            name: "Also.Never.Taken".to_owned(),
            bytes: 5_000,
            ratio: 0,
        });
        later.data.push(file(
            "/srv/media/downloads/Also.Never.Taken/c.mkv",
            5_000,
            43,
            1,
        ));
        assert_ne!(first.agreement, reckon(&later).agreement);
        assert_eq!(first.agreement.len(), 8, "{}", first.agreement);
    }

    #[test]
    fn the_stack_stands_where_its_worst_volume_stands_and_says_when_it_halts() {
        let mut measured = a_stack();
        measured.volumes.push(Volume::measured(
            Role::Services,
            &PathBuf::from("/home/op/.local/share/lemonfiber"),
            &StorageFacts {
                point: PathBuf::from("/home"),
                kind: FsKind::classify("ext4"),
                removable: false,
                available: 1_000,
                total: 1_000_000_000,
            },
            0,
            0,
        ));
        let reckoned = reckon(&measured);
        assert_eq!(reckoned.level, crate::space::Level::Exhausted);
        assert!(reckoned.halted);
        assert_eq!(reckoned.volumes.len(), 2, "both are reported");
    }

    #[test]
    fn space_freed_outside_this_product_clears_the_condition_on_the_next_reading() {
        // Nothing is kept between runs, so a disk somebody emptied by hand reads as
        // emptied rather than as whatever the last verdict was.
        let mut full = a_stack();
        full.volumes = vec![Volume::measured(
            Role::Data,
            &PathBuf::from("/srv/media"),
            &StorageFacts {
                point: PathBuf::from("/srv"),
                kind: FsKind::classify("ext4"),
                removable: false,
                available: 1_000,
                total: 1_000_000_000,
            },
            0,
            0,
        )];
        assert!(reckon(&full).halted);
        assert!(!reckon(&a_stack()).halted, "the same code, a fuller disk");
    }

    #[test]
    fn an_import_that_stopped_part_way_is_named_with_what_is_on_disk_for_it() {
        let mut measured = a_stack();
        measured.stalled = vec![
            Stalled {
                name: "Never.Taken".to_owned(),
                said: Some("No space left on device".to_owned()),
            },
            Stalled {
                name: "Unheard.Of".to_owned(),
                said: None,
            },
        ];
        let reckoned = reckon(&measured);
        assert_eq!(reckoned.interrupted.len(), 2);
        assert_eq!(reckoned.interrupted[0].partial, 3_000);
        assert_eq!(reckoned.interrupted[0].said, "No space left on device");
        assert_eq!(reckoned.interrupted[1].partial, 0);
        assert!(
            reckoned.interrupted[1].said.contains("no reason"),
            "silence is said as silence: {}",
            reckoned.interrupted[1].said
        );
    }

    #[test]
    fn a_file_sitting_in_the_root_itself_is_still_accounted_for() {
        let mut measured = a_stack();
        measured.data.push(file("/srv/media/loose.mkv", 700, 99, 1));
        let reckoned = reckon(&measured);
        assert_eq!(
            line(&reckoned.consumption, "the data location itself")
                .unwrap_or_default()
                .physical,
            700
        );
    }

    #[test]
    fn a_walked_file_outside_the_root_is_named_by_what_it_is_under() {
        // Nothing should produce one, and a walk that did must not lose it.
        let mut measured = a_stack();
        measured.data = vec![file("/elsewhere/odd/one.mkv", 5, 51, 1)];
        let reckoned = reckon(&measured);
        assert_eq!(
            line(&reckoned.consumption, "elsewhere")
                .unwrap_or_default()
                .physical,
            5
        );
    }

    #[test]
    fn a_stack_with_nothing_on_it_reports_no_empty_lines() {
        let reckoned = reckon(&Measured::default());
        assert!(reckoned.consumption.is_empty());
        assert!(reckoned.reclaimable.is_empty());
        assert!(reckoned.candidates.is_empty());
        assert!(reckoned.outsized.is_empty());
        assert!(reckoned.reclaimed.is_none());
        assert_eq!(reckoned.level, crate::space::Level::Unknown);
        assert!(!reckoned.halted);
    }

    #[test]
    fn archive_parts_beside_what_was_unpacked_from_them_are_offered() {
        let mut measured = a_stack();
        measured
            .data
            .push(file("/srv/media/downloads/Done/a.rar", 400, 61, 1));
        measured
            .data
            .push(file("/srv/media/downloads/Done/Done.mkv", 900, 62, 1));
        let reckoned = reckon(&measured);
        assert_eq!(
            line(&reckoned.reclaimable, "archives already unpacked")
                .unwrap_or_default()
                .physical,
            400
        );
        let taking: Vec<String> = reckoned
            .offering(&measured)
            .into_iter()
            .map(|occupant| occupant.path.display().to_string())
            .collect();
        assert_eq!(
            taking,
            [
                "/srv/media/downloads/Done/a.rar",
                "/srv/media/downloads/Never.Taken/b.mkv"
            ]
        );
    }

    #[test]
    fn nothing_offered_twice_where_a_download_is_also_an_unpacked_archive() {
        let mut measured = a_stack();
        measured
            .data
            .push(file("/srv/media/downloads/Never.Taken/c.rar", 400, 63, 1));
        let taking: Vec<String> = reckon(&measured)
            .offering(&measured)
            .into_iter()
            .map(|occupant| occupant.path.display().to_string())
            .collect();
        assert_eq!(
            taking,
            [
                "/srv/media/downloads/Never.Taken/b.mkv",
                "/srv/media/downloads/Never.Taken/c.rar"
            ],
            "each path once, however many reasons there are to take it"
        );
    }

    #[test]
    fn every_reclaim_code_this_module_raises_belongs_to_it() {
        for code in [
            super::HALTED,
            super::NOWHERE_TO_MEASURE,
            super::WALK_REFUSED,
        ] {
            assert!(
                code.as_str().starts_with("SPACE-"),
                "{} is this feature's own",
                code.as_str()
            );
        }
    }

    #[test]
    fn the_category_of_a_line_decides_what_reclaiming_it_costs() {
        let reckoned = reckon(&a_stack());
        for line in reckoned.consumption.iter().chain(&reckoned.reclaimable) {
            assert_eq!(line.reclaim, line.category.reclaim());
        }
        assert!(
            reckoned
                .consumption
                .iter()
                .any(|line| matches!(line.category, Category::Tree(_))),
            "there was a tree to check the rule against"
        );
    }
}
