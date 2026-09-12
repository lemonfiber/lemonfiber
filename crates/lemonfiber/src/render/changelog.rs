//! What a release changed, on a terminal.
//!
//! The summary leads and the identifiers do not. An operator reading "the VPN panel
//! now shows the forwarded port" is being told what changed; a bare identifier beside
//! it tells them nothing they can act on, and putting one first would make every line
//! read as an issue tracker. So a bullet is the sentence, and what follows it is the
//! feature the change belongs to, in words. The identifiers stay in the
//! machine-readable answer, where a reader who wants them is a program that can do
//! something with them.
//!
//! Maintenance is counted rather than listed. Nothing is dropped — the record holds
//! every entry and the machine-readable answer carries them — but sixteen dependency
//! bumps between an operator and the two changes they came to read about is how a
//! changelog stops being read at all.
//!
//! **A record that cannot be trusted is never shown as current.** Where it and this
//! build disagree about what went out, the disagreement is the first thing said and
//! the entries are shown underneath it as what the record claims rather than as what
//! shipped. Hiding them would be the other failure: a changelog that quietly drops
//! what it is unsure of teaches an operator that absence means nothing happened.

use lemonfiber_core::changelog::{Notes, Release, State, Summary};
use lemonfiber_core::plural::s;

use super::Lines;

/// The group everything that served nobody's requirement lands in.
const MAINTENANCE: &str = "Maintenance";

/// What the record says, for the version that asked it.
pub(crate) fn notes(notes: &Notes, running: &str) -> Lines {
    let mut lines = Lines::default();
    if notes.state == State::Stale {
        lines.spaced("The record of what shipped and this build disagree, so what");
        lines.put("follows is what the record claims rather than what went out.");
    }
    match &notes.running {
        Some(release) => lines.extend(changed(release, notes)),
        None => lines.extend(unwritten(notes, running)),
    }
    lines.extend(listing(&notes.releases));
    lines
}

/// What one release changed, group by group.
fn changed(release: &Release, notes: &Notes) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(headline(release));
    if let Some(delivers) = &release.delivers {
        lines.put(delivers.clone());
    }
    if let Some(reason) = &release.withdrawn {
        lines.put(format!("Withdrawn: {reason}"));
    }
    if !release.user_facing {
        lines.put("Internal only — nothing an operator would notice.");
    }
    let mut maintenance = 0;
    for group in &release.groups {
        if group.title == MAINTENANCE {
            maintenance += group.entries.len();
            continue;
        }
        lines.spaced(group.title.clone());
        for entry in &group.entries {
            lines.put(format!(
                "  • {}",
                said(&entry.summary, &served(entry, notes))
            ));
        }
    }
    if maintenance > 0 {
        lines.spaced(format!(
            "And {maintenance} maintenance change{} nobody asked for.",
            s(maintenance)
        ));
    }
    lines
}

/// The version and when it went out, as the line above what it changed.
fn headline(release: &Release) -> String {
    let patched = release
        .patches
        .as_ref()
        .map_or_else(String::new, |patched| format!(", the patch for {patched}"));
    let released = release
        .released_on
        .as_ref()
        .map_or_else(String::new, |day| format!(", released {day}"));
    format!("What {} changed{patched}{released}", release.version)
}

/// One bullet: the sentence, then the features it served.
fn said(summary: &str, features: &[String]) -> String {
    if features.is_empty() {
        return summary.to_owned();
    }
    format!("{summary} — {}", features.join(", "))
}

/// The features one entry's requirements belong to, each named once.
///
/// A requirement the specification no longer defines is named as withdrawn rather
/// than silently left off: the change did ship, and what it served having been taken
/// away since is a fact about the record rather than a reason to edit it.
fn served(entry: &lemonfiber_core::changelog::Entry, notes: &Notes) -> Vec<String> {
    let mut named: Vec<String> = Vec::new();
    for identifier in &entry.requirements {
        let Some(requirement) = notes.requirements.get(identifier) else {
            continue;
        };
        let feature = if requirement.withdrawn {
            format!("{} (withdrawn)", requirement.feature)
        } else {
            requirement.feature.clone()
        };
        if !named.contains(&feature) {
            named.push(feature);
        }
    }
    named
}

/// What is said where the record holds no release for the version asking.
fn unwritten(notes: &Notes, running: &str) -> Lines {
    let mut lines = Lines::default();
    if notes.releases.is_empty() {
        lines.spaced("This build carries no record of what any release changed.");
        return lines;
    }
    lines.spaced(format!(
        "{running} has not been released, so nothing is written about it yet."
    ));
    lines
}

/// Every release there has been, and which of them were taken back.
///
/// Counted rather than listed, and the withdrawals named rather than counted. Which
/// releases exist is a number; which of them somebody should not be running is the
/// thing they came to find out.
fn listing(releases: &[Summary]) -> Lines {
    let mut lines = Lines::default();
    let Some(oldest) = releases.last() else {
        return lines;
    };
    lines.spaced(format!(
        "{} release{} recorded, back to {}.",
        releases.len(),
        s(releases.len()),
        oldest.version
    ));
    for release in releases {
        if let Some(reason) = &release.withdrawn {
            lines.put(format!("{} was withdrawn: {reason}", release.version));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::changelog::Notes;

    use super::super::fixtures::notes as fixture;
    use super::notes;

    /// What a version's notes read as, as one text.
    fn read(running: &str) -> String {
        notes(&fixture(running), running).text()
    }

    #[test]
    fn the_summary_leads_and_the_identifiers_are_nowhere_in_it() {
        let said = read("0.4.0");
        assert!(
            said.contains("• The panel shows the forwarded port — VPN verification"),
            "{said}"
        );
        assert!(
            !said.contains("C2-R4"),
            "an identifier reached a person's report: {said}"
        );
    }

    #[test]
    fn a_requirement_withdrawn_since_is_named_as_withdrawn_rather_than_dropped() {
        // Both of the entry's requirements belong to the same feature and one of them
        // has been withdrawn since, so the feature is named twice and said once each
        // way — the change shipped, and what it served is gone.
        let said = read("0.4.0");
        assert!(
            said.contains("VPN verification, VPN verification (withdrawn)"),
            "{said}"
        );
    }

    #[test]
    fn maintenance_is_counted_and_the_count_reads_as_english() {
        let said = read("0.4.0");
        assert!(
            said.contains("And 1 maintenance change nobody asked for."),
            "{said}"
        );
    }

    #[test]
    fn the_release_says_what_it_delivered_and_when_it_went_out() {
        let said = read("0.4.0");
        assert!(
            said.contains("What 0.4.0 changed, released 2026-04-01"),
            "{said}"
        );
        assert!(said.contains("Seeing what is happening"), "{said}");
    }

    #[test]
    fn a_patch_says_which_version_it_patched_and_why_it_was_taken_back() {
        let said = read("0.3.1");
        assert!(
            said.contains("What 0.3.1 changed, the patch for 0.3.0, released 2026-03-14"),
            "{said}"
        );
        assert!(
            said.contains("Withdrawn: the installer shipped a broken pin"),
            "{said}"
        );
    }

    #[test]
    fn a_release_with_nothing_user_facing_says_so_rather_than_showing_nothing() {
        let said = read("0.3.0");
        assert!(
            said.contains("Internal only — nothing an operator would notice."),
            "{said}"
        );
        assert!(said.contains("And 1 maintenance change"), "{said}");
    }

    #[test]
    fn a_withdrawn_release_is_named_in_the_listing_whichever_version_is_asking() {
        let said = read("0.4.0");
        assert!(
            said.contains("3 releases recorded, back to 0.3.0."),
            "{said}"
        );
        assert!(
            said.contains("0.3.1 was withdrawn: the installer shipped a broken pin"),
            "{said}"
        );
    }

    #[test]
    fn a_version_with_no_release_of_its_own_is_told_so_and_still_sees_the_rest() {
        let said = read("0.5.0");
        assert!(
            said.contains("0.5.0 has not been released, so nothing is written about it yet."),
            "{said}"
        );
        assert!(said.contains("3 releases recorded"), "{said}");
    }

    #[test]
    fn a_record_that_disagrees_with_this_build_is_not_offered_as_current() {
        // Asking as a version older than the newest the record holds: the record
        // claims a release this build cannot have shipped in.
        let said = read("0.3.0");
        assert!(said.contains("disagree, so what"), "{said}");
    }

    #[test]
    fn a_build_carrying_no_record_says_that_rather_than_showing_an_empty_changelog() {
        let said = notes(&Notes::unread(), "0.4.0").text();
        assert!(
            said.contains("This build carries no record of what any release changed."),
            "{said}"
        );
        assert!(!said.contains("recorded, back to"), "{said}");
    }
}
