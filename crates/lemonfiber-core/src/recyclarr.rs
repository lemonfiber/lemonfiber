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
pub fn same_profile(kind: Kind, first: Preset, second: Preset) -> bool {
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
mod tests {
    use std::collections::BTreeSet;

    use super::{guidance, rewrite, same_profile, Kind};
    use crate::quality::{Preset, Selection};

    /// The `recyclarr.yml` that ships in the stack — its defaults are the Balanced
    /// templates, which makes it the fixture the rewriter is checked against.
    const SHIPPED: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack/config/recyclarr/recyclarr.yml"
    ));

    /// Every preset names a file the stack actually ships, for the right service.
    ///
    /// The guidance is carried here rather than fetched, so a name that is right in
    /// spelling and wrong in fact is a sync that reports nothing and exits `0` —
    /// indistinguishable from a stack with nothing to sync. Checked against the
    /// directory itself, not a list written beside the test.
    #[test]
    fn every_preset_names_an_include_the_stack_ships_for_that_service() {
        for kind in Kind::ALL {
            for preset in Preset::ALL {
                let path = guidance(kind, preset).path();
                let file = path.rsplit('/').next().unwrap_or_default();
                assert!(
                    file.starts_with(kind.section()),
                    "{path} is not a {kind:?} include"
                );
                let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../assets/media-stack/config/recyclarr/includes")
                    .join(file);
                assert!(shipped.is_file(), "{path} is named but not shipped");
            }
        }
    }

    /// The presets that differ do so by pointing somewhere else.
    ///
    /// Television offers two profiles and film four, so some presets share a file;
    /// what must not happen is every preset resolving to the same one, which would
    /// make the whole choice cosmetic.
    #[test]
    fn the_presets_that_differ_name_different_includes() {
        for kind in Kind::ALL {
            let paths: BTreeSet<_> = Preset::ALL
                .iter()
                .map(|preset| guidance(kind, *preset).path())
                .collect();
            assert!(
                paths.len() > 1,
                "{kind:?} resolves every preset to the same include: {paths:?}"
            );
        }
    }

    #[test]
    fn each_service_names_its_own_cutoff_unmet_upgrade_command() {
        // The upgrade action re-searches existing content; the command is named per
        // service, and a compose id round-trips to the right one.
        assert_eq!(
            Kind::for_section("sonarr").map(Kind::upgrade_command),
            Some("CutoffUnmetEpisodeSearch")
        );
        assert_eq!(
            Kind::for_section("radarr").map(Kind::upgrade_command),
            Some("CutoffUnmetMoviesSearch")
        );
        assert_eq!(Kind::for_section("prowlarr"), None);
    }

    #[test]
    fn each_service_names_its_own_search_and_history_endpoints() {
        // Television is searched and traced by episode; film by movie.
        assert_eq!(Kind::Sonarr.release_id_param(), "episodeId");
        assert_eq!(Kind::Radarr.release_id_param(), "movieId");
        assert_eq!(Kind::Sonarr.library_endpoint(), "series");
        assert_eq!(Kind::Radarr.library_endpoint(), "movie");
        assert_eq!(Kind::Sonarr.history_filter(), "seriesIds");
        assert_eq!(Kind::Radarr.history_filter(), "movieIds");
        // Only television files its items in parts; a film is the whole item.
        assert_eq!(Kind::Sonarr.parts_endpoint(), Some("episode"));
        assert_eq!(Kind::Radarr.parts_endpoint(), None);
        assert_eq!(Kind::Sonarr.reference_field(), "tvdbId");
        assert_eq!(Kind::Radarr.reference_field(), "tmdbId");
        assert_eq!(Kind::Sonarr.search_option(), "searchForMissingEpisodes");
        assert_eq!(Kind::Radarr.search_option(), "searchForMovie");
        assert_eq!(Kind::Sonarr.parts_filter(), "seriesId");
        assert_eq!(Kind::Radarr.parts_filter(), "movieId");
        // The words a household uses, not the services' own.
        assert_eq!(Kind::Sonarr.noun(), "series");
        assert_eq!(Kind::Radarr.noun(), "film");
    }

    #[test]
    fn the_three_1080p_presets_collapse_for_television() {
        assert!(same_profile(
            Kind::Sonarr,
            Preset::SpaceSaving,
            Preset::Balanced
        ));
        assert!(same_profile(
            Kind::Sonarr,
            Preset::Balanced,
            Preset::HighQuality
        ));
        // Only 4K stands apart for television.
        assert!(!same_profile(
            Kind::Sonarr,
            Preset::HighQuality,
            Preset::Maximum
        ));
    }

    #[test]
    fn film_keeps_all_four_presets_distinct() {
        for (first, second) in [
            (Preset::SpaceSaving, Preset::Balanced),
            (Preset::Balanced, Preset::HighQuality),
            (Preset::HighQuality, Preset::Maximum),
        ] {
            assert!(
                !same_profile(Kind::Radarr, first, second),
                "{first:?} and {second:?} should differ for film",
            );
        }
    }

    #[test]
    fn applying_balanced_everywhere_reproduces_the_shipped_default() {
        // The strongest anchor: the shipped file already carries the Balanced
        // templates, so rewriting it with Balanced must return it byte for byte —
        // proving both the mapping and that the rewriter preserves everything else.
        let balanced = Selection::everywhere(Preset::Balanced);
        assert_eq!(rewrite(SHIPPED, &balanced), SHIPPED);
    }

    #[test]
    fn every_preset_leaves_each_service_with_templates() {
        // Whatever the choice, the writer never emits an empty include list: a
        // rewritten config always carries templates for both services. This is
        // lemonfiber's half of "never fall back to unconfigured" — the config it
        // writes is always configured. (Leaving an already-synced profile intact when
        // the upstream is unreachable is Recyclarr's own behaviour, not this code's.)
        for preset in Preset::ALL {
            let config = rewrite(SHIPPED, &Selection::everywhere(preset));
            for kind in Kind::ALL {
                let included = config.lines().filter(|line| {
                    line.trim().starts_with("- config:") && line.contains(kind.section())
                });
                assert_eq!(
                    included.count(),
                    1,
                    "{preset:?} left {kind:?} misconfigured"
                );
            }
        }
    }

    #[test]
    fn maximum_swaps_both_services_to_their_4k_templates() {
        let rewritten = rewrite(SHIPPED, &Selection::everywhere(Preset::Maximum));
        assert!(rewritten.contains("sonarr-web-2160p.yml"));
        assert!(rewritten.contains("radarr-uhd-bluray-web.yml"));
        // The Balanced includes the shipped file carried are gone. `uhd-bluray-web`
        // ends in `hd-bluray-web`, so the check is against the whole file name.
        assert!(!rewritten.contains("sonarr-web-1080p.yml"));
        assert!(!rewritten.contains("/radarr-hd-bluray-web.yml"));
    }

    #[test]
    fn a_per_type_override_reaches_only_its_service() {
        let mut selection = Selection::everywhere(Preset::Balanced);
        selection.set_type("movies", Preset::Maximum);
        let rewritten = rewrite(SHIPPED, &selection);
        // Film moved to 4K; television kept the Balanced 1080p include.
        assert!(rewritten.contains("radarr-uhd-bluray-web.yml"));
        assert!(rewritten.contains("sonarr-web-1080p.yml"));
    }

    #[test]
    fn comments_addresses_and_keys_survive_a_rewrite() {
        let rewritten = rewrite(SHIPPED, &Selection::everywhere(Preset::Maximum));
        assert!(rewritten.contains("base_url: http://sonarr:8989"));
        assert!(rewritten.contains("api_key: !env_var RADARR_API_KEY"));
        assert!(rewritten.contains("# Recyclarr"));
    }

    #[test]
    fn rewriting_is_idempotent() {
        let selection = Selection::everywhere(Preset::HighQuality);
        let once = rewrite(SHIPPED, &selection);
        assert_eq!(rewrite(&once, &selection), once);
    }

    #[test]
    fn an_operators_own_section_is_left_untouched() {
        // A top-level key that is not a service: its include list must not be
        // rewritten, exercising the "no recognised section" path.
        let config = "\
lidarr:
  main:
    include:
      - template: something-of-my-own
";
        assert_eq!(
            rewrite(config, &Selection::everywhere(Preset::Balanced)),
            config
        );
    }

    #[test]
    fn an_empty_include_is_filled_at_the_derived_indent() {
        // An `include:` with no entries to copy the indent from: the entries are
        // written two levels below the `include:` key.
        let config = "\
sonarr:
  main:
    include:
radarr:
";
        let rewritten = rewrite(config, &Selection::everywhere(Preset::Balanced));
        assert!(rewritten.contains("      - config: /config/includes/sonarr-web-1080p.yml"));
    }

    #[test]
    fn no_stale_include_survives_a_reshaped_include_block() {
        // The block-of-interest case: an operator has put a blank line and a
        // comment among the entries, and an older preset left more than one. Every
        // `- config:` must still be replaced — wherever it sits — so no
        // previous-preset entry lingers.
        let config = "\
radarr:
  main:
    include:

      - config: /config/includes/radarr-hd-bluray-web.yml
      # the one I keep meaning to revisit
      - config: /config/includes/radarr-sqp-1-web-1080p.yml
";
        let rewritten = rewrite(config, &Selection::everywhere(Preset::Maximum));
        // The 4K include is present, and not one older entry remains.
        assert!(rewritten.contains("radarr-uhd-bluray-web.yml"));
        assert!(!rewritten.contains("/radarr-hd-bluray-web.yml"));
        assert!(!rewritten.contains("radarr-sqp-1-web-1080p.yml"));
        // The blank line and the operator's comment are untouched.
        assert!(rewritten.contains("\n\n"));
        assert!(rewritten.contains("# the one I keep meaning to revisit"));
    }

    #[test]
    fn an_entry_this_does_not_manage_is_kept_across_a_rewrite() {
        // A `- template:` names something the sync tool fetches for itself. It is
        // the operator's, not this product's, and must survive while the include
        // beside it is replaced.
        let config = "\
sonarr:
  main:
    include:
      - template: something-of-my-own
      - config: /config/includes/sonarr-web-1080p.yml
";
        let rewritten = rewrite(config, &Selection::everywhere(Preset::Maximum));
        assert!(rewritten.contains("- template: something-of-my-own"));
        assert!(rewritten.contains("sonarr-web-2160p.yml"));
        assert!(!rewritten.contains("sonarr-web-1080p.yml"));
    }

    #[test]
    fn a_section_key_carrying_an_inline_comment_is_still_recognised() {
        // `sonarr: # note` is a valid key; the section — and so its include — must
        // still be found, rather than silently skipped.
        let config = "\
sonarr: # primary television instance
  main:
    include: # profiles
      - config: /config/includes/sonarr-web-1080p.yml
";
        let rewritten = rewrite(config, &Selection::everywhere(Preset::Maximum));
        assert!(rewritten.contains("sonarr-web-2160p.yml"));
        // The commented key line itself is preserved verbatim.
        assert!(rewritten.contains("sonarr: # primary television instance"));
    }
}
