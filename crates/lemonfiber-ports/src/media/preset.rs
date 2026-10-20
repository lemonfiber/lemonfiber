//! How good the operator wants their video to look, as four presets in plain terms.

use serde::{Deserialize, Serialize};

/// How good the operator wants their media to look, and how much disk they will
/// spend on it — chosen without ever learning what a custom format is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Preset {
    /// Good enough on a laptop or tablet: 720p–1080p, smaller encodes.
    SpaceSaving,
    /// Looks right on a TV, at sensible file sizes: 1080p, good encodes. The one
    /// chosen when the operator expresses no preference.
    Balanced,
    /// The best 1080p available, with size a secondary concern: 1080p, high-bitrate.
    HighQuality,
    /// 4K where it exists, with HDR preserved: 2160p, very large files.
    Maximum,
}

/// What choosing a preset practically costs: the resolution it targets, roughly how
/// much disk an hour of it takes, and whether a typical client will have to
/// transcode it — the three things the choice actually decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Consequence {
    /// The resolution and encode the preset targets, in plain terms.
    pub resolution: &'static str,
    /// Roughly how much disk an hour of content at this preset takes.
    pub size_per_hour: &'static str,
    /// What playback costs: whether content at this preset plays directly on most
    /// devices, or commonly needs transcoding — and what that means.
    pub transcoding: &'static str,
}

impl Preset {
    /// Every preset, in the order they are offered — least to most demanding.
    pub const ALL: [Self; 4] = [
        Self::SpaceSaving,
        Self::Balanced,
        Self::HighQuality,
        Self::Maximum,
    ];

    /// The preset chosen when the operator expresses no preference: looks right on a
    /// television at sensible file sizes.
    #[must_use]
    pub const fn default_preset() -> Self {
        Self::Balanced
    }

    /// The plain-language name an operator selects the preset by, and that it is
    /// stored under — never a custom-format or scoring term.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SpaceSaving => "space-saving",
            Self::Balanced => "balanced",
            Self::HighQuality => "high-quality",
            Self::Maximum => "maximum",
        }
    }

    /// The preset an operator named, or `None` where the name is not one of the four
    /// — so a mistyped choice is refused rather than guessed.
    #[must_use]
    pub fn from_label(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.label() == name)
    }

    /// What the preset means, in the operator's terms rather than the tool's.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::SpaceSaving => "Good enough on a laptop or tablet.",
            Self::Balanced => "Looks right on a TV, at sensible file sizes.",
            Self::HighQuality => "The best 1080p available; size is secondary.",
            Self::Maximum => "4K where it exists, with HDR preserved.",
        }
    }

    /// What the preset practically costs — resolution, size per hour, and the
    /// transcoding it implies — so the choice is made knowing its consequence.
    #[must_use]
    pub const fn consequence(self) -> Consequence {
        match self {
            Self::SpaceSaving => Consequence {
                resolution: "720p–1080p, smaller encodes",
                size_per_hour: "about 0.5–1 GB per hour",
                transcoding: "plays directly on virtually any device",
            },
            Self::Balanced => Consequence {
                resolution: "1080p, good encodes",
                size_per_hour: "about 2–3 GB per hour",
                transcoding: "plays directly on most devices",
            },
            Self::HighQuality => Consequence {
                resolution: "1080p, high-bitrate",
                size_per_hour: "about 4–6 GB per hour",
                transcoding: "plays directly on most devices; a weak client may transcode",
            },
            Self::Maximum => Consequence {
                resolution: "2160p (4K), HDR preserved",
                size_per_hour: "about 10–25 GB per hour",
                transcoding: "4K HDR often needs transcoding on a device that cannot play \
                    it directly, which is CPU-bound without hardware support",
            },
        }
    }

    /// Whether choosing this preset commonly forces the media server to transcode
    /// — true only for the 4K HDR maximum, the one whose [`Consequence`] calls
    /// transcoding a likely cost. It is the gate a host without hardware
    /// transcoding warns on, so the plain boolean lives beside the prose it agrees
    /// with rather than being read back out of that prose.
    #[must_use]
    pub const fn likely_needs_transcoding(self) -> bool {
        matches!(self, Self::Maximum)
    }

    /// Roughly how many bytes an hour of content at this preset takes — the midpoint
    /// of the range [`Consequence::size_per_hour`] states in words, a representative
    /// figure rather than a guarantee. Real sizes vary with the source; what a
    /// storage projection turns on is the ratio between presets, which the midpoints
    /// preserve without the systematic over-estimate an upper bound would carry.
    #[must_use]
    pub const fn bytes_per_hour(self) -> u64 {
        // Decimal gigabytes, to read the same as the "GB per hour" the operator sees.
        // The midpoint of each stated range: 0.5–1 → 0.75, 2–3 → 2.5, 4–6 → 5,
        // 10–25 → 17.5.
        match self {
            Self::SpaceSaving => 750_000_000,
            Self::Balanced => 2_500_000_000,
            Self::HighQuality => 5_000_000_000,
            Self::Maximum => 17_500_000_000,
        }
    }
}
