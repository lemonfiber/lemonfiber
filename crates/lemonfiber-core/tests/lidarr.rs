//! The Lidarr quality-profile rewrite, driven through its public functions.
//!
//! The rewrite is pure, but it is also reached from the Servarr client's music-quality
//! apply, which the integration tests exercise. So it is driven from here rather than
//! from an in-crate test: a module tested both in-crate and through an integration
//! binary is compiled twice, and its line coverage is then counted from the wrong copy.

use lemonfiber_core::audio::Format;
use lemonfiber_core::lidarr::{
    hi_res_custom_format, prefers_hi_res, rewrite, set_hi_res_preference, HI_RES_FORMAT,
};
use serde_json::Value;

/// Lidarr's default group names, which the rewrite matches on — kept here as the
/// literals a real profile carries, so a test reads the same names an operator sees.
const HIGH_QUALITY_LOSSY: &str = "High Quality Lossy";
const LOSSLESS: &str = "Lossless";

/// A Lidarr default-shaped profile: the two groups a format speaks to, an ungrouped
/// leaf (WAV) and the Unknown leaf that should never be wanted, a cutoff pointing at
/// Lossless, and a stray field to prove the rewrite preserves it.
const PROFILE: &str = r#"{
    "name":"Standard","upgradeAllowed":false,"cutoff":1006,
    "minFormatScore":0,"cutoffFormatScore":0,"formatItems":[],
    "items":[
        {"quality":{"id":0,"name":"Unknown"},"items":[],"allowed":false},
        {"id":1005,"name":"High Quality Lossy","allowed":false,"items":[
            {"quality":{"id":19,"name":"MP3-VBR-V0"},"items":[],"allowed":false},
            {"quality":{"id":4,"name":"MP3-320"},"items":[],"allowed":false},
            {"quality":{"id":11,"name":"AAC-320"},"items":[],"allowed":false}
        ]},
        {"id":1006,"name":"Lossless","allowed":false,"items":[
            {"quality":{"id":6,"name":"FLAC"},"items":[],"allowed":false},
            {"quality":{"id":7,"name":"ALAC"},"items":[],"allowed":false},
            {"quality":{"id":21,"name":"FLAC 24bit"},"items":[],"allowed":false},
            {"quality":{"id":37,"name":"ALAC 24bit"},"items":[],"allowed":false}
        ]},
        {"quality":{"id":13,"name":"WAV"},"items":[],"allowed":false}
    ]
}"#;

/// The parsed rewrite, or an empty object if it refused — so a test reads a field
/// without unwrapping.
fn rewritten(format: Format) -> Value {
    let json = rewrite(PROFILE, format).unwrap_or_default();
    serde_json::from_str(&json).unwrap_or_default()
}

/// The score a named format carries in a profile's format items, if present.
fn format_score(profile: &Value, name: &str) -> Option<i64> {
    profile
        .get("formatItems")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(name))
        .and_then(|item| item.get("score"))
        .and_then(Value::as_i64)
}

/// Whether the group of the given name is allowed in a rewritten profile.
fn group_allowed(profile: &Value, group: &str) -> bool {
    profile
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(group))
        .and_then(|item| item.get("allowed"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[test]
fn compact_allows_only_high_bitrate_lossy_and_cuts_off_there() {
    let profile = rewritten(Format::Compact);
    assert!(group_allowed(&profile, HIGH_QUALITY_LOSSY));
    assert!(
        !group_allowed(&profile, LOSSLESS),
        "compact does not grab lossless — the cost it avoids"
    );
    assert_eq!(profile.get("cutoff").and_then(Value::as_i64), Some(1005));
}

#[test]
fn lossless_allows_lossy_as_a_fallback_and_cuts_off_at_lossless() {
    // Both groups on, so a lossless choice grabs high-bitrate lossy when no lossless
    // release exists yet and upgrades to lossless later; cutoff is the lossless group.
    let profile = rewritten(Format::Lossless);
    assert!(group_allowed(&profile, HIGH_QUALITY_LOSSY));
    assert!(group_allowed(&profile, LOSSLESS));
    assert_eq!(profile.get("cutoff").and_then(Value::as_i64), Some(1006));
}

#[test]
fn hi_res_shares_the_lossless_allow_axis() {
    // On the quality axis hi-res is lossless — Lidarr cannot separate 24-bit within the
    // group. The 24-bit preference is a format score set elsewhere; here the two are the
    // same allow-set and cutoff.
    let lossless = rewritten(Format::Lossless);
    let hi_res = rewritten(Format::HiRes);
    assert_eq!(lossless.get("items"), hi_res.get("items"));
    assert_eq!(lossless.get("cutoff"), hi_res.get("cutoff"));
}

#[test]
fn nested_leaves_follow_their_group() {
    // The grab decision reads the group's flag, but Lidarr keeps a group and its leaves
    // in step; the rewrite does the same so the profile reads consistently.
    let profile = rewritten(Format::Lossless);
    let lossless_leaves_allowed = profile
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(LOSSLESS))
        .and_then(|group| group.get("items"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .all(|leaf| leaf.get("allowed").and_then(Value::as_bool) == Some(true));
    assert!(lossless_leaves_allowed);
}

#[test]
fn an_ungrouped_leaf_is_never_wanted() {
    // WAV and Unknown sit outside the groups a format speaks to, so no format allows
    // them — a rewrite that left one on would grab a quality nobody chose.
    for format in Format::ALL {
        let profile = rewritten(format);
        let stray_allowed = profile
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|item| item.get("name").is_none())
            .any(|leaf| leaf.get("allowed").and_then(Value::as_bool) == Some(true));
        assert!(!stray_allowed, "a format wanted an ungrouped leaf");
    }
}

#[test]
fn every_format_turns_upgrading_on() {
    for format in Format::ALL {
        let profile = rewritten(format);
        assert_eq!(
            profile.get("upgradeAllowed").and_then(Value::as_bool),
            Some(true)
        );
    }
}

#[test]
fn fields_the_rewrite_does_not_touch_are_preserved() {
    // The profile's name and a score field it never reasons about survive, so sending it
    // back changes only quality wanting — not everything else Lidarr set.
    let profile = rewritten(Format::Compact);
    assert_eq!(
        profile.get("name").and_then(Value::as_str),
        Some("Standard")
    );
    assert_eq!(
        profile.get("cutoffFormatScore").and_then(Value::as_i64),
        Some(0)
    );
}

#[test]
fn a_cutoff_group_the_profile_lacks_leaves_the_cutoff_as_it_was() {
    // A profile with no Lossless group, asked for lossless: the allow-axis applies to the
    // groups that are present, but with nothing to point the cutoff at, it is left as
    // Lidarr had it rather than guessed at.
    const NO_LOSSLESS: &str = r#"{"upgradeAllowed":false,"cutoff":1005,"items":[
        {"id":1005,"name":"High Quality Lossy","allowed":false,"items":[
            {"quality":{"id":4,"name":"MP3-320"},"items":[],"allowed":false}
        ]}
    ]}"#;
    let json = rewrite(NO_LOSSLESS, Format::Lossless).unwrap_or_default();
    let profile: Value = serde_json::from_str(&json).unwrap_or_default();
    assert_eq!(profile.get("cutoff").and_then(Value::as_i64), Some(1005));
}

#[test]
fn a_garbled_profile_is_refused_rather_than_written_back() {
    assert_eq!(rewrite("not json", Format::Lossless), None);
    assert_eq!(rewrite("{}", Format::Lossless), None);
}

#[test]
fn only_hi_res_prefers_24_bit() {
    assert!(prefers_hi_res(Format::HiRes));
    assert!(!prefers_hi_res(Format::Lossless));
    assert!(!prefers_hi_res(Format::Compact));
}

#[test]
fn the_24_bit_custom_format_matches_a_release_title() {
    // It is created as a release-title regex — the one signal Lidarr exposes for 24-bit —
    // carrying lemonfiber's name so it is recognised as ours.
    let body: Value = serde_json::from_str(&hi_res_custom_format()).unwrap_or_default();
    assert_eq!(
        body.get("name").and_then(Value::as_str),
        Some(HI_RES_FORMAT)
    );
    let implementation = body
        .get("specifications")
        .and_then(Value::as_array)
        .and_then(|specs| specs.first())
        .and_then(|spec| spec.get("implementation"))
        .and_then(Value::as_str);
    assert_eq!(implementation, Some("ReleaseTitleSpecification"));
}

/// A profile carrying the 24-bit format in its format items at a zero score, as Lidarr
/// lists every custom format once it exists — alongside another format it leaves alone,
/// so the preference is set on ours and not on every entry.
const WITH_FORMAT: &str = r#"{"cutoffFormatScore":0,"formatItems":[
    {"format":3,"name":"Someone Else's Format","score":25},
    {"format":9,"name":"lemonfiber: 24-bit","score":0}
]}"#;

#[test]
fn preferring_hi_res_scores_the_format_and_the_cutoff() {
    let profile: Value = set_hi_res_preference(WITH_FORMAT, true)
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    assert!(profile.get("cutoffFormatScore").and_then(Value::as_i64) > Some(0));
    assert!(
        format_score(&profile, HI_RES_FORMAT) > Some(0),
        "the 24-bit format was scored"
    );
    // Another format the operator scored is left exactly as it was.
    assert_eq!(format_score(&profile, "Someone Else's Format"), Some(25));
}

#[test]
fn not_preferring_hi_res_clears_the_score_so_switching_away_drops_it() {
    let scored = set_hi_res_preference(WITH_FORMAT, true).unwrap_or_default();
    let cleared: Value = set_hi_res_preference(&scored, false)
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    assert_eq!(
        cleared.get("cutoffFormatScore").and_then(Value::as_i64),
        Some(0)
    );
}

#[test]
fn a_cutoff_score_is_never_raised_beyond_a_format_that_can_reach_it() {
    // A profile that does not yet carry the 24-bit format (it was not created): the
    // preference must not raise the cutoff score, or Lidarr would search forever for a
    // score nothing scores.
    const NO_FORMAT: &str = r#"{"cutoffFormatScore":0,"formatItems":[]}"#;
    let profile: Value = set_hi_res_preference(NO_FORMAT, true)
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    assert_eq!(
        profile.get("cutoffFormatScore").and_then(Value::as_i64),
        Some(0)
    );
}

#[test]
fn a_preference_on_a_non_profile_is_refused() {
    assert_eq!(set_hi_res_preference("not json", true), None);
    assert_eq!(set_hi_res_preference("[]", true), None);
}
