//! Carrying an [`audio::Format`](crate::audio) out against Lidarr's own quality profile.
//!
//! The resolution presets lean on a community profile (`TRaSH` via Recyclarr); music
//! has none — Recyclarr configures only Sonarr and Radarr — so a format is applied
//! straight to Lidarr's quality profile through its API. This module is the pure part
//! of that: which of Lidarr's quality groups a format allows, which is its cutoff, and
//! a dependency-free rewrite of a fetched profile to match. Sending it is a separate
//! concern; nothing here reaches a service.
//!
//! Lidarr enforces quality by *group*, not by individual quality: its grab decision
//! reads the top-level group's `allowed` flag and ignores a nested quality's own, and
//! its comparer ranks by group, so FLAC and FLAC-24 — both in the one "Lossless"
//! group — are indistinguishable on the quality axis. So the allow-axis has two rungs,
//! compact and lossless; preferring 24-bit within lossless is a custom-format score,
//! applied where that format is created rather than here.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::audio::Format;

/// Lidarr's group of high-bitrate lossy qualities — MP3/AAC around 320 kbps. The
/// most an operator saving space wants, and the fallback a lossless choice accepts
/// while it waits for a lossless release.
const HIGH_QUALITY_LOSSY: &str = "High Quality Lossy";

/// Lidarr's group of lossless qualities — FLAC and ALAC, including their 24-bit
/// variants, which Lidarr keeps in this one group.
const LOSSLESS: &str = "Lossless";

/// The Lidarr quality groups a format allows a release to satisfy, in the order
/// Lidarr lists them. A lossless choice also allows high-bitrate lossy, so something
/// is grabbed when no lossless release exists yet and upgraded to lossless later; a
/// compact choice allows only the lossy group, since grabbing lossless is the cost it
/// is avoiding.
const fn allowed_groups(format: Format) -> &'static [&'static str] {
    match format {
        Format::Compact => &[HIGH_QUALITY_LOSSY],
        Format::Lossless | Format::HiRes => &[HIGH_QUALITY_LOSSY, LOSSLESS],
    }
}

/// The group a format's cutoff sits in — the point at or above which Lidarr stops
/// searching for something better. Compact is satisfied by high-bitrate lossy;
/// lossless (and hi-res, which shares the allow-axis) by the lossless group.
const fn cutoff_group(format: Format) -> &'static str {
    match format {
        Format::Compact => HIGH_QUALITY_LOSSY,
        Format::Lossless | Format::HiRes => LOSSLESS,
    }
}

/// One entry in a Lidarr quality profile: a leaf quality, or a group carrying nested
/// leaves. Only the fields this rewrite touches are named; everything else Lidarr
/// carries — the quality's `id`/`name`, sizes, anything a version adds — is preserved
/// verbatim so the profile sent back differs from the one fetched only where intended.
#[derive(Deserialize, Serialize)]
struct Item {
    /// A group's id, absent on a leaf quality. Lidarr's cutoff names a group by it.
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    /// A group's name, absent on a leaf quality — how a group is matched to a format.
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// A leaf's quality, absent on a group. Preserved, not interpreted.
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<Value>,
    /// A group's nested leaves; empty on a leaf.
    #[serde(default)]
    items: Vec<Item>,
    /// Whether a release of this group or quality is wanted — the flag Lidarr's grab
    /// decision actually reads.
    allowed: bool,
    /// Everything else on the entry, preserved across the rewrite.
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// A Lidarr quality profile, in the shape its API returns and accepts. Only the
/// upgrade flag, cutoff, and items are rewritten; the rest — name, id, format items,
/// scores — round-trips untouched.
#[derive(Deserialize, Serialize)]
struct Profile {
    /// Whether Lidarr keeps searching for a better release after a first grab. A
    /// format sets this on, so a lossless choice that first grabbed lossy upgrades.
    #[serde(rename = "upgradeAllowed")]
    upgrade_allowed: bool,
    /// The id of the group (or quality) at or above which searching stops.
    cutoff: i64,
    /// The quality groups and leaves, in Lidarr's order.
    items: Vec<Item>,
    /// Everything else on the profile, preserved across the rewrite.
    #[serde(flatten)]
    rest: Map<String, Value>,
}

impl Item {
    /// Set this entry and any leaves it groups to `allowed`, mirroring Lidarr's own
    /// rule that a group and its members share the flag.
    fn set_allowed(&mut self, allowed: bool) {
        self.allowed = allowed;
        for nested in &mut self.items {
            nested.allowed = allowed;
        }
    }
}

/// Rewrite a fetched Lidarr quality profile to carry out a format: allow exactly the
/// groups the format wants (and no others), point the cutoff at the format's group,
/// and turn upgrading on. Returns `None` if the text is not a profile Lidarr would
/// have sent, so a garbled read is refused rather than written back malformed.
///
/// Matching is by Lidarr's default group names, the same assumption the Recyclarr
/// rewrite makes of its section keys; a profile whose groups an operator has renamed
/// is left to them. The 24-bit preference a hi-res choice adds is not here — it is a
/// custom-format score, set where that format is created.
#[must_use]
pub fn rewrite(profile_json: &str, format: Format) -> Option<String> {
    let mut profile: Profile = serde_json::from_str(profile_json).ok()?;
    let allowed = allowed_groups(format);
    let cutoff = cutoff_group(format);

    for item in &mut profile.items {
        let wanted = item
            .name
            .as_deref()
            .is_some_and(|name| allowed.contains(&name));
        item.set_allowed(wanted);
    }

    if let Some(id) = profile
        .items
        .iter()
        .find(|item| item.name.as_deref() == Some(cutoff))
        .and_then(|item| item.id)
    {
        profile.cutoff = id;
    }
    profile.upgrade_allowed = true;

    serde_json::to_string(&profile).ok()
}

#[cfg(test)]
mod tests {
    use super::{rewrite, HIGH_QUALITY_LOSSY, LOSSLESS};
    use crate::audio::Format;
    use serde_json::Value;

    /// A Lidarr default-shaped profile: the two groups a format speaks to, an
    /// ungrouped leaf (WAV) and the Unknown leaf that should never be wanted, a cutoff
    /// pointing at Lossless, and a stray field to prove the rewrite preserves it.
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
        // On the quality axis hi-res is lossless — Lidarr cannot separate 24-bit within
        // the group. The 24-bit preference is a format score set elsewhere; here the two
        // are the same allow-set and cutoff.
        let lossless = rewritten(Format::Lossless);
        let hi_res = rewritten(Format::HiRes);
        assert_eq!(lossless.get("items"), hi_res.get("items"));
        assert_eq!(lossless.get("cutoff"), hi_res.get("cutoff"));
    }

    #[test]
    fn nested_leaves_follow_their_group() {
        // The grab decision reads the group's flag, but Lidarr keeps a group and its
        // leaves in step; the rewrite does the same so the profile reads consistently.
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
        // The profile's name and a score field it never reasons about survive, so sending
        // it back changes only quality wanting — not everything else Lidarr set.
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
        // A profile with no Lossless group, asked for lossless: the allow-axis applies to
        // the groups that are present, but with nothing to point the cutoff at, it is left
        // as Lidarr had it rather than guessed at.
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
}
