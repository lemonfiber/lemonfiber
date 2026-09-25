//! Quality presets in plain language.
//!
//! Quality configuration is the deepest rabbit hole in this ecosystem: custom
//! formats, scoring, release groups, repack handling — an evening's work and real
//! domain knowledge to do by hand. But the operator's actual question is small:
//! *how good should this look, and how much disk am I willing to spend?*
//!
//! This module is that question as four presets, each stating in plain terms what
//! it means and what it costs — an approximate size per hour and whether a typical
//! client will have to transcode it, because that is what the choice actually buys.
//! Carrying a preset out is a separate concern: the presets are a friendly surface
//! over the community-maintained profiles ([TRaSH](https://trash-guides.info) via
//! Recyclarr), so the hard part stays maintained upstream and this only translates
//! the question. Nothing here reaches a service or a disk; it is the pure model the
//! surface and the Recyclarr writer are built on.

pub(crate) mod run;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::audio::Format;

/// How good the operator wants their media to look, and how much disk they will
/// spend on it — chosen without ever learning what a custom format is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    pub(crate) const fn default_preset() -> Self {
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
    pub(crate) const fn likely_needs_transcoding(self) -> bool {
        matches!(self, Self::Maximum)
    }

    /// Roughly how many bytes an hour of content at this preset takes — the midpoint
    /// of the range [`Consequence::size_per_hour`] states in words, a representative
    /// figure rather than a guarantee. Real sizes vary with the source; what a
    /// storage projection turns on is the ratio between presets, which the midpoints
    /// preserve without the systematic over-estimate an upper bound would carry.
    #[must_use]
    pub(crate) const fn bytes_per_hour(self) -> u64 {
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

/// The operator's quality choice across their media: one preset for everything, with
/// per-media-type exceptions where they want a type treated differently — Maximum
/// for film and Balanced for television is the common split, since a series is many
/// times the volume of a film.
///
/// The choice is forward-looking: it decides what is acquired next, never a
/// retroactive rewrite of what is already on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// The preset in force where a media type has no exception of its own.
    global: Preset,
    /// Media types the operator chose a different preset for, keyed by type.
    #[serde(default)]
    per_type: BTreeMap<String, Preset>,
    /// The audio format chosen for music, where one has been — a separate axis, since
    /// music has no resolution to set. Absent until the operator chooses; an old
    /// choice file that predates it reads as absent and falls back to the default.
    #[serde(default)]
    music: Option<Format>,
}

impl Selection {
    /// A fresh choice: one preset everywhere, no per-type exceptions yet.
    #[must_use]
    pub fn everywhere(global: Preset) -> Self {
        Self {
            global,
            per_type: BTreeMap::new(),
            music: None,
        }
    }

    /// The audio format in force for music: the operator's choice where they made one,
    /// otherwise the default. Music is a separate axis from the resolution presets, so
    /// it is read apart from them rather than through [`Selection::for_type`].
    #[must_use]
    pub fn music(&self) -> Format {
        self.music.unwrap_or_else(Format::default_format)
    }

    /// Whether an audio format has been chosen for music — as opposed to the default
    /// standing in. Lets a surface show music only where it is genuinely set.
    #[must_use]
    pub(crate) fn music_chosen(&self) -> bool {
        self.music.is_some()
    }

    /// Choose the audio format for music.
    pub(crate) fn set_music(&mut self, format: Format) {
        self.music = Some(format);
    }

    /// The preset in force everywhere a media type has no exception of its own.
    #[must_use]
    pub fn global(&self) -> Preset {
        self.global
    }

    /// The preset in force for a media type: its own exception where it has one,
    /// otherwise the global choice.
    #[must_use]
    pub(crate) fn for_type(&self, media_type: &str) -> Preset {
        self.per_type
            .get(media_type)
            .copied()
            .unwrap_or(self.global)
    }

    /// The media types set apart from the global choice, each with its preset —
    /// only the genuine exceptions, so a redundant entry equal to the global is
    /// not reported as one.
    pub fn overrides(&self) -> impl Iterator<Item = (&str, Preset)> {
        self.per_type
            .iter()
            .filter(|(_, preset)| **preset != self.global)
            .map(|(media_type, preset)| (media_type.as_str(), *preset))
    }

    /// Change the preset in force where a type has no exception. Any exception that
    /// now matches the new global stops being one, so raising everything to a
    /// preset a type was already set to leaves no redundant override behind.
    pub(crate) fn set_global(&mut self, preset: Preset) {
        self.global = preset;
        self.per_type.retain(|_, exception| *exception != preset);
    }

    /// Set a media type's preset apart from the global choice. Setting it to the
    /// global preset is not an exception, so it is cleared rather than stored — a
    /// type only appears as an override while it genuinely differs.
    pub(crate) fn set_type(&mut self, media_type: &str, preset: Preset) {
        if preset == self.global {
            self.per_type.remove(media_type);
        } else {
            self.per_type.insert(media_type.to_owned(), preset);
        }
    }

    /// Whether any media type is set apart from the global choice — the `overridden`
    /// state, as opposed to one preset applying across the board. Read from the data
    /// rather than from the map being non-empty, so a redundant entry equal to the
    /// global — which a deserialized or hand-edited selection can carry, bypassing
    /// [`Self::set_type`]'s pruning — is not mistaken for a genuine exception.
    #[must_use]
    pub fn is_overridden(&self) -> bool {
        self.per_type.values().any(|preset| *preset != self.global)
    }

    /// The preset in force that costs the most disk per hour — the global choice, or
    /// a hungrier per-type exception where one is set. It is the basis for a storage
    /// projection: if any media is kept at a demanding preset, the disk has to
    /// accommodate that rate, so the warning turns on the hungriest choice rather
    /// than an average that would understate the risk.
    #[must_use]
    pub(crate) fn most_demanding(&self) -> Preset {
        self.per_type
            .values()
            .copied()
            .chain(std::iter::once(self.global))
            .max_by_key(|preset| preset.bytes_per_hour())
            .unwrap_or(self.global)
    }
}

#[cfg(test)]
mod tests;
