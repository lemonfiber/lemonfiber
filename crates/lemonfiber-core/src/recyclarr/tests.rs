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
