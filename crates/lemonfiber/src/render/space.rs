//! Where the disk went, on a terminal.
//!
//! Ordered by what somebody came here to find out. When it runs out is first,
//! because that is the question — "how much is free" is only ever asked as a way
//! of asking it. Where the room went is second. What can be got back is third, and
//! it is last of the three because it is the part somebody acts on, and an action
//! read before its reason is an action taken without one.
//!
//! Each reclaimable line says what taking it costs in the same breath as what it
//! would free, and never in a paragraph afterwards. A figure with its cost a screen
//! away is a figure somebody decides on without the cost.

use lemonfiber_core::bytes::humanize;
use lemonfiber_core::space::{
    ratio_reads, Candidate, Consumption, Freshness, Reckoning, Reclaimed, Standing, Volume,
};

use super::Lines;

/// Where the disk stands, and what became of an answer to it.
pub(crate) fn reckoning(report: &Reckoning) -> Lines {
    let mut lines = Lines::default();
    for volume in &report.volumes {
        lines.extend(standing(volume));
    }
    if report.halted {
        lines.spaced("Nothing new is being fetched: a service that cannot write its database");
        lines.put("can lose it, and that is what stopping protects.");
    }
    lines.extend(went(&report.consumption));
    lines.extend(back(&report.reclaimable));
    lines.extend(named(&report.candidates));
    lines.extend(oversized(report));
    lines.extend(stopped(report));
    lines.extend(ending(report));
    lines
}

/// One volume: where it stands, what is left, and what is already spoken for.
fn standing(volume: &Volume) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(format!("{} — {}", volume.role.word(), volume.level.means()));
    lines.put(format!("  at    {}", volume.at));
    match (volume.free, volume.limit) {
        (Some(free), Some(limit)) => {
            lines.put(format!("  free  {} of {}", humanize(free), humanize(limit)));
        }
        _ => lines.put("  free  could not be read"),
    }
    if volume.committed > 0 {
        lines.put(format!(
            "  queued {} still to land, leaving {}",
            humanize(volume.committed),
            volume
                .projected
                .map_or_else(|| "an amount nobody could work out".to_owned(), humanize)
        ));
    }
    if volume.level.worth_saying() {
        lines.put(format!("  if it fills  {}", volume.role.costs()));
    }
    // A figure read across a share is what the share last said, and nothing here can
    // make it fresher — so it is dated rather than presented as now.
    if let Freshness::AsOf(taken) = volume.reading {
        lines.put(format!(
            "  read  across a network share, as it stood at {taken} seconds past the epoch"
        ));
    }
    lines
}

/// Where the room went.
fn went(consumption: &[Consumption]) -> Lines {
    let mut lines = Lines::default();
    if consumption.is_empty() {
        return lines;
    }
    lines.spaced("Where the room went:");
    for line in consumption {
        lines.put(format!(
            "  {:<28} {}",
            line.category.heading(),
            reading(line)
        ));
    }
    lines
}

/// What could be got back, what each would cost, and which of it an answer takes.
///
/// Which lines a yes applies to is marked here rather than left to be worked out
/// from the costs: an operator about to agree to something is entitled to see the
/// scope of what they are agreeing to on the same screen as the figures, and a
/// reader who inferred it from "costs nothing" would be inferring a policy.
fn back(reclaimable: &[Consumption]) -> Lines {
    let mut lines = Lines::default();
    if reclaimable.is_empty() {
        return lines;
    }
    lines.spaced("What could be got back:");
    for line in reclaimable {
        lines.put(format!(
            "  {:<28} {}",
            line.category.heading(),
            reading(line)
        ));
        let taken = if line.reclaim.offered() {
            " — and this is what --confirm takes"
        } else {
            ""
        };
        lines.put(format!("  {:<28} {}{taken}", "", line.reclaim.says()));
    }
    lines
}

/// One line's figures: what it occupies, and what it would occupy unshared where
/// the two differ.
fn reading(line: &Consumption) -> String {
    if line.tally.differs() {
        return format!(
            "{} on the disk ({} unshared, {} saved by linking)",
            humanize(line.tally.physical),
            humanize(line.tally.logical),
            humanize(line.tally.saved())
        );
    }
    humanize(line.tally.physical)
}

/// The completed downloads, each with where it stands.
fn named(candidates: &[Candidate]) -> Lines {
    let mut lines = Lines::default();
    if candidates.is_empty() {
        return lines;
    }
    lines.spaced("The completed downloads:");
    for candidate in candidates {
        lines.put(format!(
            "  {} — {}",
            candidate.name,
            humanize(candidate.bytes)
        ));
        lines.put(format!("    {}", stands(candidate)));
        if let Some(consequence) = &candidate.consequence {
            lines.put(format!("    {consequence}"));
        }
    }
    lines
}

/// Where one download stands, in the words its standing is always said in.
fn stands(candidate: &Candidate) -> String {
    match candidate.standing {
        Standing::NeverImported => {
            "nothing ever linked this into a library, so removing it loses nothing".to_owned()
        }
        // A torrent added from files already on disk downloaded nothing, so there is
        // no ratio to divide — the fact is said rather than the number that stands
        // for it, which would read as forty-two million.
        Standing::Seeding { ratio } => ratio_reads(ratio).map_or_else(
            || "imported, and still seeding, having given back more than it ever took".to_owned(),
            |reads| format!("imported, and still seeding at a ratio of {reads}"),
        ),
        Standing::LeftAlone => "you asked for this one to be left alone".to_owned(),
    }
}

/// The files far enough out of line with the rest to be worth pointing at.
fn oversized(report: &Reckoning) -> Lines {
    let mut lines = Lines::default();
    if report.outsized.is_empty() {
        return lines;
    }
    lines.spaced("Far larger than anything else here, which is rarely on purpose:");
    for one in &report.outsized {
        lines.put(format!(
            "  {} — {}, {} times the middle file",
            one.path,
            humanize(one.bytes),
            one.times_typical
        ));
    }
    lines
}

/// The imports that stopped part-way.
fn stopped(report: &Reckoning) -> Lines {
    let mut lines = Lines::default();
    if report.interrupted.is_empty() {
        return lines;
    }
    lines.spaced("Stopped part-way, and still on the disk:");
    for one in &report.interrupted {
        lines.put(format!("  {} — {}", one.name, one.said));
        if one.partial > 0 {
            lines.put(format!("    {} written so far", humanize(one.partial)));
        }
    }
    lines.put("Free room before retrying these, or the retry stops in the same place.");
    lines
}

/// What a confirmed run took, or what one would take.
fn ending(report: &Reckoning) -> Lines {
    let mut lines = Lines::default();
    let Some(taken) = &report.reclaimed else {
        lines.spaced(offer(report));
        return lines;
    };
    lines.extend(took(taken));
    lines
}

/// What is on offer, where nothing has been taken.
///
/// Read off the lines that are actually on offer rather than off there being
/// anything reclaimable at all: a disk whose only reclaimable room is a torrent
/// still seeding has nothing an answer would take, and inviting one would be
/// inviting an answer to a question nothing here asks.
fn offer(report: &Reckoning) -> String {
    if report.reclaimable.iter().any(|line| line.reclaim.offered()) {
        return "Nothing was removed. Add --confirm to take the lines marked above — never a \
                torrent still seeding, and never anything you asked to be left alone."
            .to_owned();
    }
    "Nothing here can be got back for free.".to_owned()
}

/// What a confirmed run took, and what it could not.
fn took(taken: &Reclaimed) -> Lines {
    let mut lines = Lines::default();
    if taken.gone.is_empty() {
        lines.spaced("Nothing was removed.");
    } else {
        lines.spaced(format!("Removed, freeing {}:", humanize(taken.bytes)));
        for at in &taken.gone {
            lines.put(format!("  {at}"));
        }
    }
    if taken.left.is_empty() {
        return lines;
    }
    lines.spaced("Still here, and each will have to be removed by hand:");
    for still in &taken.left {
        lines.put(format!("  {} — {}", still.at, still.why));
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use lemonfiber_core::ports::filesystem::{FsKind, Identity, StorageFacts};
    use lemonfiber_core::ports::occupancy::Occupant;
    use lemonfiber_core::ports::service::Seeded;
    use lemonfiber_core::space::{
        reckon, Left, Measured, Reckoning, Reclaimed, Role, Stalled, Volume,
    };

    use super::reckoning;

    /// A walked file with a given number of names pointing at it.
    fn file(path: &str, bytes: u64, inode: u64, links: u64) -> Occupant {
        Occupant {
            path: PathBuf::from(path),
            bytes,
            identity: Some(Identity { file: inode, links }),
        }
    }

    /// A volume with the given room left, reached the given way.
    fn volume(role: Role, at: &str, available: u64, kind: &str, committed: u64) -> Volume {
        Volume::measured(
            role,
            &PathBuf::from(at),
            &StorageFacts {
                point: PathBuf::from("/srv"),
                kind: FsKind::classify(kind),
                removable: false,
                available,
                total: 1_000_000_000_000,
            },
            committed,
            1_700_000_000,
        )
    }

    /// A stack whose disk holds one imported file and one nothing ever took.
    fn a_stack() -> Measured {
        Measured {
            volumes: vec![volume(
                Role::Data,
                "/srv/media",
                40_000_000_000,
                "ext4",
                35_000_000_000,
            )],
            root: PathBuf::from("/srv/media"),
            data: vec![
                file("/srv/media/downloads/Imported/a.mkv", 8_000, 41, 2),
                file("/srv/media/films/Imported/a.mkv", 8_000, 41, 2),
                file("/srv/media/downloads/Never.Taken/b.mkv", 3_000, 42, 1),
            ],
            services: Vec::new(),
            landing: 35_000_000_000,
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

    /// What the report reads as.
    fn said(reckoned: &Reckoning) -> String {
        reckoning(reckoned).text()
    }

    #[test]
    fn when_it_runs_out_is_read_before_anything_else() {
        let text = said(&reckon(&a_stack()));
        let projected = text.find("still to land").unwrap_or(usize::MAX);
        let went = text.find("Where the room went").unwrap_or(0);
        assert!(
            projected < went,
            "the question somebody came here for is first:\n{text}"
        );
        assert!(text.contains("the data location"), "{text}");
    }

    #[test]
    fn a_shared_file_reads_as_both_figures_and_what_the_sharing_saved() {
        let text = said(&reckon(&a_stack()));
        assert!(text.contains("on the disk"), "{text}");
        assert!(text.contains("saved by linking"), "{text}");
    }

    #[test]
    fn which_lines_an_answer_takes_is_marked_beside_them_rather_than_inferred() {
        // The scope of what is being agreed to belongs on the same screen as the
        // figures. The line that costs nothing carries the mark; the one that costs
        // a tracker's opinion does not.
        let text = said(&reckon(&a_stack()));
        let marked: Vec<&str> = text
            .lines()
            .filter(|line| line.contains("what --confirm takes"))
            .collect();
        assert_eq!(marked.len(), 1, "{text}");
        assert!(
            marked
                .first()
                .is_some_and(|line| line.contains("nothing ever took these")),
            "{text}"
        );
    }

    #[test]
    fn what_removing_a_seeding_torrent_costs_is_beside_it_rather_than_afterwards() {
        let text = said(&reckon(&a_stack()));
        let ratio = text.find("ratio of 1.75").unwrap_or(usize::MAX);
        let cost = text.find("cost the account").unwrap_or(usize::MAX);
        assert!(ratio < cost, "the cost follows the figure:\n{text}");
        assert!(
            text.contains("nothing ever linked this into a library"),
            "and the one that costs nothing says so:\n{text}"
        );
    }

    #[test]
    fn nothing_is_removed_without_an_answer_and_the_offer_says_what_it_excludes() {
        let text = said(&reckon(&a_stack()));
        assert!(text.contains("Nothing was removed."), "{text}");
        assert!(text.contains("--confirm"), "{text}");
        assert!(text.contains("never a torrent still seeding"), "{text}");
    }

    #[test]
    fn a_full_disk_says_what_stopping_protects() {
        let mut measured = a_stack();
        measured.volumes = vec![volume(Role::Data, "/srv/media", 1_000, "ext4", 0)];
        let text = said(&reckon(&measured));
        assert!(text.contains("Nothing new is being fetched"), "{text}");
        assert!(text.contains("database"), "{text}");
        assert!(text.contains("if it fills"), "{text}");
    }

    #[test]
    fn a_reading_across_a_share_is_dated_rather_than_presented_as_now() {
        let mut measured = a_stack();
        measured.volumes = vec![volume(Role::Data, "/mnt/nas", 40_000_000_000, "nfs", 0)];
        let text = said(&reckon(&measured));
        assert!(text.contains("across a network share"), "{text}");
        assert!(text.contains("1700000000"), "{text}");
    }

    #[test]
    fn a_volume_nobody_could_read_says_so_rather_than_showing_a_figure() {
        let mut measured = a_stack();
        measured.volumes = vec![Volume::measured(
            Role::Data,
            &PathBuf::from("/srv/media"),
            &StorageFacts {
                point: PathBuf::new(),
                kind: FsKind::classify(""),
                removable: false,
                available: 0,
                total: 0,
            },
            0,
            0,
        )];
        let text = said(&reckon(&measured));
        assert!(text.contains("could not be read"), "{text}");
    }

    #[test]
    fn an_import_that_stopped_says_to_free_room_before_retrying() {
        let mut measured = a_stack();
        measured.stalled = vec![Stalled {
            name: "Never.Taken".to_owned(),
            said: Some("No space left on device".to_owned()),
        }];
        let text = said(&reckon(&measured));
        assert!(text.contains("Stopped part-way"), "{text}");
        assert!(text.contains("No space left on device"), "{text}");
        assert!(text.contains("written so far"), "{text}");
        assert!(text.contains("before retrying"), "{text}");
    }

    #[test]
    fn an_import_that_stopped_with_nothing_written_names_no_figure() {
        let mut measured = a_stack();
        measured.stalled = vec![Stalled {
            name: "Unheard.Of".to_owned(),
            said: None,
        }];
        let text = said(&reckon(&measured));
        assert!(text.contains("Unheard.Of"), "{text}");
        assert!(!text.contains("written so far"), "{text}");
    }

    #[test]
    fn a_confirmed_run_says_what_went_and_what_would_not() {
        let taken = Reckoning {
            reclaimed: Some(Reclaimed {
                gone: vec!["/srv/media/downloads/Never.Taken/b.mkv".to_owned()],
                bytes: 3_000,
                left: vec![Left {
                    at: "/srv/media/downloads/Held/a.rar".to_owned(),
                    why: "permission denied".to_owned(),
                }],
            }),
            ..reckon(&a_stack())
        };
        let text = said(&taken);
        assert!(text.contains("Removed, freeing"), "{text}");
        assert!(text.contains("removed by hand"), "{text}");
        assert!(text.contains("permission denied"), "{text}");
    }

    #[test]
    fn a_confirmed_run_that_took_nothing_says_nothing_went() {
        let taken = Reckoning {
            reclaimed: Some(Reclaimed {
                gone: Vec::new(),
                bytes: 0,
                left: Vec::new(),
            }),
            ..reckon(&a_stack())
        };
        assert!(said(&taken).contains("Nothing was removed."));
    }

    #[test]
    fn a_disk_with_nothing_reclaimable_says_so_rather_than_offering_nothing() {
        let text = said(&reckon(&Measured::default()));
        assert!(
            text.contains("Nothing here can be got back for free."),
            "{text}"
        );
    }

    #[test]
    fn what_the_operator_asked_to_be_left_alone_says_so_and_carries_no_ratio() {
        let mut measured = a_stack();
        measured.marked = BTreeSet::from(["Never.Taken".to_owned()]);
        let text = said(&reckon(&measured));
        assert!(
            text.contains("you asked for this one to be left alone"),
            "{text}"
        );
        assert!(text.contains("left alone at your request"), "{text}");
        assert!(
            !text.contains("what --confirm takes"),
            "nothing on offer, so nothing is marked as taken: {text}"
        );
    }

    #[test]
    fn a_torrent_that_downloaded_nothing_is_said_rather_than_shown_as_a_number() {
        let mut measured = a_stack();
        measured.held = vec![Seeded {
            name: "Imported".to_owned(),
            bytes: 8_000,
            ratio: u32::MAX,
        }];
        let text = said(&reckon(&measured));
        assert!(text.contains("given back more than it ever took"), "{text}");
        assert!(!text.contains("ratio of 42"), "{text}");
    }

    #[test]
    fn a_disk_whose_only_reclaimable_room_is_seeding_invites_no_answer() {
        // Everything here is reclaimable and none of it is on offer, which is the
        // case that would read as an invitation if the offer were decided by there
        // being anything reclaimable at all.
        let mut measured = a_stack();
        measured.data = vec![file("/srv/media/downloads/Imported/a.mkv", 8_000, 41, 2)];
        measured.held = vec![Seeded {
            name: "Imported".to_owned(),
            bytes: 8_000,
            ratio: 175,
        }];
        let text = said(&reckon(&measured));
        assert!(text.contains("still seeding"), "{text}");
        assert!(
            text.contains("Nothing here can be got back for free."),
            "{text}"
        );
        assert!(!text.contains("--confirm"), "{text}");
    }

    #[test]
    fn a_file_far_out_of_line_with_the_rest_is_pointed_at() {
        let mut measured = a_stack();
        let floor = 20 * 1024 * 1024 * 1024;
        for number in 0..9 {
            measured.data.push(file(
                &format!("/srv/media/films/ordinary-{number}.mkv"),
                floor / 20,
                100 + number,
                1,
            ));
        }
        measured
            .data
            .push(file("/srv/media/films/A.Remux.mkv", floor * 2, 200, 1));
        let text = said(&reckon(&measured));
        assert!(text.contains("Far larger than anything else"), "{text}");
        assert!(text.contains("A.Remux.mkv"), "{text}");
        assert!(text.contains("times the middle file"), "{text}");
    }

    #[test]
    fn a_projection_nobody_could_work_out_says_so() {
        // A volume whose free space could not be read still has a queue committed to
        // it, and the line about that queue must not invent what will be left.
        let unreadable = Volume {
            projected: None,
            committed: 5_000,
            ..volume(Role::Data, "/srv/media", 0, "", 0)
        };
        let mut measured = a_stack();
        measured.volumes = vec![unreadable];
        assert!(said(&reckon(&measured)).contains("nobody could work out"));
    }
}
