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

/// Whether a format prefers 24-bit within the lossless it allows — true only for
/// hi-res. Since Lidarr cannot separate 24-bit on the quality axis, this preference
/// is carried by a custom-format score rather than the allow-axis.
#[must_use]
pub const fn prefers_hi_res(format: Format) -> bool {
    matches!(format, Format::HiRes)
}

/// The name lemonfiber gives the custom format it creates to prefer 24-bit lossless.
/// Prefixed so it reads as lemonfiber's own and is matched back by name when the
/// preference is set or cleared, rather than mistaken for one an operator made.
pub const HI_RES_FORMAT: &str = "lemonfiber: 24-bit";

/// The score the 24-bit custom format carries when preferred. Any positive figure
/// makes a 24-bit release rank above an otherwise-equal 16-bit one and keeps Lidarr
/// searching for it; the exact value only matters relative to other scores, and
/// lemonfiber sets no others.
const HI_RES_SCORE: i64 = 100;

/// The body that creates the 24-bit custom format in Lidarr: a release-title match for
/// the ways a 24-bit release commonly names itself.
///
/// Matching the title is a heuristic — not every 24-bit release says so, and Lidarr
/// exposes no truer signal — but a missed match only leaves a release at its parsed
/// quality rather than doing harm, and a caught one is what lets the score prefer it.
#[must_use]
pub fn hi_res_custom_format() -> String {
    serde_json::json!({
        "name": HI_RES_FORMAT,
        "includeCustomFormatWhenRenaming": false,
        "specifications": [{
            "name": "24-bit",
            "implementation": "ReleaseTitleSpecification",
            "negate": false,
            "required": true,
            "fields": [{
                "name": "value",
                "value": r"(?i)\b(24[ ._-]?bit|bit[ ._-]?24|FLAC[ ._-]?24)\b"
            }]
        }]
    })
    .to_string()
}

/// Set or clear the 24-bit preference on a fetched profile: score the 24-bit custom
/// format and set the profile's cutoff score to match, so Lidarr upgrades toward 24-bit
/// and keeps searching until it has it — or, when not preferred, clear both so switching
/// away from hi-res drops the preference rather than leaving it to churn.
///
/// The format is matched by name in the profile's format items, where Lidarr lists every
/// custom format; a profile that does not yet carry it (the format not created) is left
/// with its cutoff score cleared, never raised, so it cannot ask for a score nothing can
/// reach. Returns `None` only if the text is not a profile.
#[must_use]
pub fn set_hi_res_preference(profile_json: &str, prefer: bool) -> Option<String> {
    let mut profile: Value = serde_json::from_str(profile_json).ok()?;
    let Value::Object(object) = &mut profile else {
        return None;
    };

    let mut scored = false;
    if let Some(Value::Array(items)) = object.get_mut("formatItems") {
        for entry in items.iter_mut().filter_map(Value::as_object_mut) {
            if entry.get("name").and_then(Value::as_str) == Some(HI_RES_FORMAT) {
                let score = if prefer { HI_RES_SCORE } else { 0 };
                entry.insert("score".to_owned(), score.into());
                scored = prefer;
            }
        }
    }

    // Only ask for a cutoff score the profile can actually reach: raise it when the
    // format was there to score, clear it otherwise.
    let cutoff = if scored { HI_RES_SCORE } else { 0 };
    object.insert("cutoffFormatScore".to_owned(), cutoff.into());

    serde_json::to_string(&profile).ok()
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
