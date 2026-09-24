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
    ratio_reads, Candidate, Consumption, Freshness, Letting, Reckoning, Reclaimed, Standing, Volume,
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

/// What letting one completed download go would cost, and what became of it.
///
/// The cost first and the answer last, which is the order the account beside it is
/// read in and the order that matters most here: the name to answer with is under the
/// consequence, so nobody reaches it without passing what it costs.
pub(crate) fn letting(offer: &Letting) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} — {}",
        offer.download.name,
        humanize(offer.download.bytes)
    ));
    lines.put(format!("  {}", stands(&offer.download)));
    if let Some(consequence) = &offer.download.consequence {
        lines.put(format!("  {consequence}"));
    }
    lines.put(format!("  {}", offer.goes));
    match &offer.gone {
        Some(gone) if gone.rehearsed => lines.spaced(format!(
            "Nothing was asked. {} and its {} would go.",
            gone.name,
            humanize(gone.bytes)
        )),
        Some(gone) => lines.spaced(format!(
            "The client has let {} go, and it is no longer seeding.",
            gone.name
        )),
        None => {
            lines.spaced("Nothing has been removed. To go ahead, answer this offer by name:");
            lines.put(format!(
                "  lemonfiber stop-seeding \"{}\" --offer {}",
                offer.download.name, offer.agreement
            ));
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
mod tests;
