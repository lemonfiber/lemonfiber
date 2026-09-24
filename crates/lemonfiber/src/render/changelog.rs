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

/// What the release these pins came from changed, and nothing else.
///
/// The narrower reading, for a report that is about something other than the
/// changelog. Which releases there have been is the version read's answer; somebody
/// weighing a stack update is asking about the one that brought the stack they are
/// being offered, and a list of every release under it would bury the reason they
/// came.
///
/// Silent where the record holds no release for this build, which is what a build
/// between tags is. Saying "nothing is written yet" would be a sentence about the
/// changelog inside a report about the stack.
pub(crate) fn brought(notes: &Notes) -> Lines {
    let mut lines = Lines::default();
    let Some(release) = &notes.running else {
        return lines;
    };
    if notes.state == State::Stale {
        lines.spaced("The record of what shipped and this build disagree, so what");
        lines.put("follows is what the record claims rather than what went out.");
    }
    lines.extend(changed(release, notes));
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

/// Notes written somewhere else, as a terminal can read them.
///
/// The release page's own words, flattened. They were generated by this project's
/// renderer and are markdown, which a terminal shows as punctuation an operator has
/// to read past: a heading is three hashes, a citation is a label and a URL in
/// brackets, and the review it came from is a number at the end of the sentence.
///
/// Flattening rather than parsing, deliberately. Nothing here assumes a shape: a line
/// this does not recognise is shown as it was written, so notes from a release older
/// than this renderer — or newer than it — still read as text rather than as an error.
pub(crate) fn flattened(markdown: &str) -> Lines {
    let mut lines = Lines::default();
    for raw in markdown.lines() {
        // The release heading names the version, which whoever is showing these has
        // just said in a sentence of their own.
        if raw.starts_with("## ") {
            continue;
        }
        lines.put(flatten(raw));
    }
    lines
}

/// One line of them.
fn flatten(line: &str) -> String {
    let line = line.trim_end();
    if let Some(heading) = line.strip_prefix("### ") {
        return plainly(heading);
    }
    if let Some(bullet) = line.strip_prefix("- ") {
        return format!("  • {}", plainly(bullet));
    }
    plainly(line.strip_prefix("> ").unwrap_or(line))
}

/// The markup taken out of one line, and the forge reference with it.
fn plainly(text: &str) -> String {
    unlinked(without_review(text))
        .replace("**", "")
        .replace("~~", "")
}

/// The same line without the review it came from, where it ends with one.
///
/// A pull request number is the way back to a discussion, which is worth having on a
/// page somebody can click and is noise in the middle of a terminal report.
fn without_review(text: &str) -> &str {
    let Some(rest) = text.strip_suffix(')') else {
        return text;
    };
    let Some((before, number)) = rest.rsplit_once(" (#") else {
        return text;
    };
    if !number.is_empty() && number.chars().all(|digit| digit.is_ascii_digit()) {
        before
    } else {
        text
    }
}

/// Links reduced to what they said, since a terminal cannot follow one.
///
/// A bracket that opens no link keeps its bracket and the scan carries on past it,
/// rather than the rest of the line being given up on. Prose written by people has
/// brackets in it, and a line holding one of those and a link would otherwise keep
/// the whole URL.
fn unlinked(text: &str) -> String {
    let mut said = String::new();
    let mut rest = text;
    loop {
        let Some((before, after)) = rest.split_once('[') else {
            said.push_str(rest);
            return said;
        };
        said.push_str(before);
        let Some((label, tail)) = link(after) else {
            said.push('[');
            rest = after;
            continue;
        };
        said.push_str(label);
        rest = tail;
    }
}

/// What a link says and what follows it, where what comes next is a link at all.
///
/// The label runs to the first `]`, which is what keeps an earlier bracket from
/// being swallowed into one: `[0] and a [link](url)` holds one link, not a label
/// eleven characters long.
fn link(after: &str) -> Option<(&str, &str)> {
    let (label, tail) = after.split_once(']')?;
    let (_, tail) = tail.strip_prefix('(')?.split_once(')')?;
    Some((label, tail))
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
mod tests;
