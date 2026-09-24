//! Carrying a quality preset out, as the file the sync tool reads it from.
//!
//! [`crate::quality`] holds the operator's question — how good, how much disk — in
//! their own words. This holds the answer in the tool's words: the TRaSH-guide
//! quality definition, profile and format groups that a preset maps to. Keeping the
//! two apart is the point of the feature: a preset never learns what a custom
//! format is, and the scoring that rots stays upstream where it is tended.
//!
//! **A preset resolves to one file the stack carries**, named by a service's
//! `include:` list in `recyclarr.yml` as a `- config:` entry. Applying a selection
//! is rewriting that entry and nothing else: [`rewrite`] leaves every comment,
//! address and key untouched, and the tool syncs the change on its own schedule.
//! This module is pure — it maps and it rewrites text; it never reaches the tool or
//! a disk.
//!
//! It used to name three templates per service that the tool fetched for itself,
//! from a registry upstream has since withdrawn — its templates are whole
//! configurations to be copied now, which is not something a stack can include. So
//! what each preset asks for is carried in the stack beside the file naming it, and
//! nothing is fetched while a sync runs. An unpinned repository cloned on every run
//! is a pinned image somebody else can break, which is how that went.
//!
//! A `- template:` entry, if an operator adds one, is theirs: this touches only the
//! `- config:` entries it put there.

pub use lemonfiber_ports::media::Kind;

use crate::quality::{Preset, Selection};

/// The file a preset's guidance is shipped in, as `recyclarr.yml` names it.
///
/// One include per preset, holding the quality definition, the profile and the
/// format groups that preset asks the guides for. It used to be three entries
/// naming templates the sync tool fetched; upstream withdrew the registry those
/// were reachable through, so the stack carries them and this names the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guidance(&'static str);

impl Guidance {
    /// Where the file sits, as the sync tool reads it from inside its container.
    #[must_use]
    pub const fn path(self) -> &'static str {
        self.0
    }
}

/// What a preset asks the guides for, for one service.
///
/// Television has fewer meaningful tiers than film: the guides offer series only a
/// WEB-1080p and a WEB-2160p profile, so the three 1080p presets resolve to the
/// same television guidance — a series in Bluray remux is impractical, and
/// presenting a distinction the upstream guides do not draw would be dishonest.
/// Film has the full range, from a streaming-sized profile through Bluray to 4K.
/// Where two presets land on the same file for a service, [`same_profile`] lets a
/// surface collapse them rather than present a choice that changes nothing.
#[must_use]
pub const fn guidance(kind: Kind, preset: Preset) -> Guidance {
    match (kind, preset) {
        // Television: only WEB-1080p and WEB-2160p exist, so the 1080p presets
        // are one and the same.
        (Kind::Sonarr, Preset::SpaceSaving | Preset::Balanced | Preset::HighQuality) => {
            Guidance("/config/includes/sonarr-web-1080p.yml")
        }
        (Kind::Sonarr, Preset::Maximum) => Guidance("/config/includes/sonarr-web-2160p.yml"),
        // Film: a streaming-sized profile, the Bluray+WEB default, a 1080p remux,
        // then 4K Bluray+WEB.
        (Kind::Radarr, Preset::SpaceSaving) => {
            Guidance("/config/includes/radarr-sqp-1-web-1080p.yml")
        }
        (Kind::Radarr, Preset::Balanced) => Guidance("/config/includes/radarr-hd-bluray-web.yml"),
        (Kind::Radarr, Preset::HighQuality) => {
            Guidance("/config/includes/radarr-remux-web-1080p.yml")
        }
        (Kind::Radarr, Preset::Maximum) => Guidance("/config/includes/radarr-uhd-bluray-web.yml"),
    }
}

/// Whether two presets ask for the same guidance for a service, so a surface
/// can collapse a distinction without a difference rather than offer both — the
/// three 1080p television presets being the case that arises in practice.
#[must_use]
pub(crate) fn same_profile(kind: Kind, first: Preset, second: Preset) -> bool {
    guidance(kind, first) == guidance(kind, second)
}

/// The indent two levels below a service — where `- template:` entries sit —
/// derived from where the `include:` key sits, for an `include:` that arrives
/// with no entries of its own to copy.
fn deeper_indent(include_indent: &str) -> String {
    format!("{include_indent}  ")
}

/// The width of a line's leading whitespace, for telling one indent level from
/// another.
fn indent_width(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// `line` with any trailing inline comment removed. A YAML comment opens at a `#`
/// preceded by whitespace, so a key or value carrying one is still recognised for
/// what it is rather than silently unmatched.
fn without_comment(line: &str) -> &str {
    match line.find(" #") {
        Some(at) => &line[..at],
        None => line,
    }
}

/// The section a top-level key names — the bare key, with its colon and any inline
/// comment stripped.
fn section_key(line: &str) -> &str {
    without_comment(line).trim_end().trim_end_matches(':')
}

/// The leading whitespace of `line` where it is the `include:` key, marking the
/// start of a service's template list — tolerating an inline comment after it.
fn include_indent(line: &str) -> Option<&str> {
    let body = line.trim_start();
    (without_comment(body).trim_end() == "include:").then(|| &line[..line.len() - body.len()])
}

/// The leading whitespace of `line` where it is a `- config:` entry, so an entry
/// can be recognised and replaced wherever it sits in the block.
///
/// Only this kind is touched. An include block may hold others — a `- template:`
/// naming something the sync tool fetches for itself — and those are the
/// operator's, left exactly where they are.
fn template_indent(line: &str) -> Option<&str> {
    let body = line.trim_start();
    body.starts_with("- config:")
        .then(|| &line[..line.len() - body.len()])
}

/// Whether `line` opens a top-level section — a key in the first column, not a
/// comment — after which entries belong to that section until the next one.
fn top_level_key(line: &str) -> bool {
    line.chars()
        .next()
        .is_some_and(|first| !first.is_whitespace() && first != '#')
}

/// Rewrite the `include:` lists of a `recyclarr.yml` so each service carries the
/// preset the selection chose for it, and leave everything else in place — the
/// comments, the addresses, the keys, and any non-template include entries such
/// as a local `- config:`.
///
/// Only the `- template:` entries under a recognised service's `include:` are
/// touched, and every one of them is, wherever it sits in the block — so no stale
/// entry from a previous preset survives even in a file an operator has since
/// reshaped. Line endings are normalised to LF and the result ends in a single
/// newline; the file this manages ships that way, and this is the one place it is
/// rewritten wholesale.
#[must_use]
pub fn rewrite(config: &str, selection: &Selection) -> String {
    let mut out = String::with_capacity(config.len());
    let mut section: Option<Kind> = None;
    let mut lines = config.lines().peekable();

    while let Some(line) = lines.next() {
        if top_level_key(line) {
            section = Kind::for_section(section_key(line));
            push_line(&mut out, line);
            continue;
        }

        match (section, include_indent(line)) {
            (Some(kind), Some(indent)) => {
                push_line(&mut out, line);
                rewrite_include_block(&mut out, &mut lines, indent, kind, selection);
            }
            _ => push_line(&mut out, line),
        }
    }

    out
}

/// Replace the `- template:` entries of the include block the iterator is now
/// inside — everything blank or indented past the `include:` key — with the ones
/// the selection calls for, and keep every other line of the block as it was.
///
/// The new entries are written at the first existing entry's indent, or two levels
/// below `include:` where the block held none, and land at that first entry's
/// position — or, where there was none, after the block's other lines.
fn rewrite_include_block<'a>(
    out: &mut String,
    lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
    include_indent: &str,
    kind: Kind,
    selection: &Selection,
) {
    let mut block = Vec::new();
    while let Some(line) =
        lines.next_if(|next| next.trim().is_empty() || indent_width(next) > include_indent.len())
    {
        block.push(line);
    }

    let entry_indent = block
        .iter()
        .find_map(|line| template_indent(line))
        .map_or_else(|| deeper_indent(include_indent), str::to_owned);
    let asked = guidance(kind, selection.for_type(kind.media_type()));
    let mut written = false;
    for line in &block {
        if template_indent(line).is_some() {
            if !written {
                push_include(out, &entry_indent, asked);
                written = true;
            }
        } else {
            push_line(out, line);
        }
    }
    if !written {
        push_include(out, &entry_indent, asked);
    }
}

/// Write the preset's guidance as the block's one `- config:` entry at `indent`.
fn push_include(out: &mut String, indent: &str, asked: Guidance) {
    push_line(out, &format!("{indent}- config: {}", asked.path()));
}

/// Append `line` and the newline `str::lines` stripped.
fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

#[cfg(test)]
mod tests;
