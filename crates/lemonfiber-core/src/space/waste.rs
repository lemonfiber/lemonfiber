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
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::{candidates, ratio_reads, Standing, RATIO_CONSEQUENCE};
    use crate::ports::filesystem::Identity;
    use crate::ports::occupancy::Occupant;
    use crate::ports::service::Seeded;

    /// A completed download the client is holding.
    fn held(name: &str, bytes: u64, ratio: u32) -> Seeded {
        Seeded {
            name: name.to_owned(),
            bytes,
            ratio,
        }
    }

    /// A walked file with a given number of names pointing at it.
    fn file(path: &str, bytes: u64, inode: u64, links: u64) -> Occupant {
        Occupant {
            path: PathBuf::from(path),
            bytes,
            identity: Some(Identity { file: inode, links }),
        }
    }

    /// Nothing has been marked to be left alone.
    fn nothing_marked() -> BTreeSet<String> {
        BTreeSet::new()
    }

    /// The one download in these cases has been.
    fn marked() -> BTreeSet<String> {
        BTreeSet::from(["A.Show.S01E01".to_owned()])
    }

    #[test]
    fn a_download_with_one_name_was_never_imported() {
        // One name means nothing ever linked it into a library, which is the
        // evidence — not a guess from the client having finished with it.
        let found = candidates(
            &[held("A.Show.S01E01", 8_000, 0)],
            &BTreeSet::new(),
            &nothing_marked(),
            &[file(
                "/srv/media/downloads/A.Show.S01E01/a.mkv",
                8_000,
                41,
                1,
            )],
        );
        assert_eq!(found.len(), 1);
        let one = found.first();
        assert!(
            one.is_some_and(|candidate| candidate.standing == Standing::NeverImported
                && candidate.offered()
                && candidate.consequence.is_none()),
            "nothing is lost by removing it, so nothing is said about weighing it: {one:?}"
        );
    }

    #[test]
    fn a_download_with_a_second_name_was_imported_and_is_seeding() {
        let found = candidates(
            &[held("A.Show.S01E01", 8_000, 175)],
            &BTreeSet::new(),
            &nothing_marked(),
            &[file(
                "/srv/media/downloads/A.Show.S01E01/a.mkv",
                8_000,
                41,
                2,
            )],
        );
        let one = found.first();
        assert!(
            one.is_some_and(
                |candidate| candidate.standing == Standing::Seeding { ratio: 175 }
                    && !candidate.offered()
                    && candidate.consequence.as_deref() == Some(RATIO_CONSEQUENCE)
            ),
            "a consequence outside this machine is not this product's to weigh: {one:?}"
        );
    }

    #[test]
    fn what_removing_a_seeding_torrent_costs_is_said_in_what_it_does() {
        let said = RATIO_CONSEQUENCE.to_lowercase();
        assert!(said.contains("stops it seeding"), "{said}");
        assert!(said.contains("ratio"), "{said}");
        assert!(
            said.contains("account"),
            "what losing the ratio actually costs: {said}"
        );
    }

    #[test]
    fn a_download_a_service_is_still_waiting_for_is_never_called_waste() {
        // One name and an *arr still queued for it is an import that has not
        // happened yet, not one that never will.
        let awaited = BTreeSet::from(["A.Show.S01E01".to_owned()]);
        let found = candidates(
            &[held("A.Show.S01E01", 8_000, 20)],
            &awaited,
            &nothing_marked(),
            &[file(
                "/srv/media/downloads/A.Show.S01E01/a.mkv",
                8_000,
                41,
                1,
            )],
        );
        let one = found.first();
        assert!(
            one.is_some_and(|candidate| !candidate.offered()
                && candidate.standing == Standing::Seeding { ratio: 20 }),
            "{one:?}"
        );
    }

    #[test]
    fn what_the_operator_asked_to_be_left_alone_is_never_offered() {
        let found = candidates(
            &[held("A.Show.S01E01", 8_000, 0)],
            &BTreeSet::new(),
            &marked(),
            &[file(
                "/srv/media/downloads/A.Show.S01E01/a.mkv",
                8_000,
                41,
                1,
            )],
        );
        let one = found.first();
        assert!(
            one.is_some_and(|candidate| candidate.standing == Standing::LeftAlone
                && !candidate.offered()
                && candidate
                    .consequence
                    .as_deref()
                    .is_some_and(|said| said.contains("left alone"))),
            "it says whose instruction it is following: {one:?}"
        );
    }

    #[test]
    fn a_ratio_reads_as_the_client_would_write_it() {
        assert_eq!(ratio_reads(175).as_deref(), Some("1.75"));
        assert_eq!(ratio_reads(0).as_deref(), Some("0.00"));
        assert_eq!(ratio_reads(7).as_deref(), Some("0.07"));
        assert_eq!(ratio_reads(1_000).as_deref(), Some("10.00"));
        assert_eq!(
            ratio_reads(u32::MAX),
            None,
            "a torrent that downloaded nothing has no ratio to divide"
        );
    }

    #[test]
    fn a_download_no_file_could_be_matched_to_is_left_out_rather_than_guessed_at() {
        // "I could not find it" and "nothing points at it" must not read alike,
        // because one of them is a reason to delete something.
        let found = candidates(
            &[held("A.Show.S01E01", 8_000, 0)],
            &BTreeSet::new(),
            &nothing_marked(),
            &[file("/srv/media/films/Something.Else.mkv", 8_000, 41, 1)],
        );
        assert!(found.is_empty());
    }

    #[test]
    fn a_download_named_as_the_file_itself_is_matched_by_its_stem() {
        let found = candidates(
            &[held("A.Film.2019", 9_000, 0)],
            &BTreeSet::new(),
            &nothing_marked(),
            &[file("/srv/media/downloads/A.Film.2019.mkv", 9_000, 42, 1)],
        );
        assert_eq!(found.len(), 1);
        assert!(found
            .first()
            .is_some_and(|one| one.standing == Standing::NeverImported));
    }

    #[test]
    fn a_file_the_platform_would_not_identify_is_not_evidence_of_a_link() {
        let unidentified = Occupant {
            path: PathBuf::from("/srv/media/downloads/A.Show.S01E01/a.mkv"),
            bytes: 8_000,
            identity: None,
        };
        let found = candidates(
            &[held("A.Show.S01E01", 8_000, 0)],
            &BTreeSet::new(),
            &nothing_marked(),
            &[unidentified],
        );
        let one = found.first();
        assert!(
            one.is_some_and(|candidate| candidate.standing == Standing::NeverImported),
            "no second name was found, and absence of a reading is not a link: {one:?}"
        );
    }

    #[test]
    fn the_largest_is_read_first_and_a_tie_is_broken_by_name() {
        let found = candidates(
            &[
                held("Bee", 1_000, 0),
                held("Cee", 9_000, 0),
                held("Ape", 1_000, 0),
            ],
            &BTreeSet::new(),
            &nothing_marked(),
            &[
                file("/d/Bee/a.mkv", 1_000, 1, 1),
                file("/d/Cee/a.mkv", 9_000, 2, 1),
                file("/d/Ape/a.mkv", 1_000, 3, 1),
            ],
        );
        let order: Vec<&str> = found.iter().map(|one| one.name.as_str()).collect();
        assert_eq!(order, ["Cee", "Ape", "Bee"]);
    }
}
