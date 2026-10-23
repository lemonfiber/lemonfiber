//! Quality presets in plain language.
//!
//! Quality configuration is the deepest rabbit hole in this ecosystem: custom
//! formats, scoring, release groups, repack handling — an evening's work and real
//! domain knowledge to do by hand. But the operator's actual question is small:
//! *how good should this look, and how much disk am I willing to spend?*
//!
//! This module is that question as four presets, each stating in plain terms what it means
//! and what it costs — an approximate size per hour and whether a typical client will have
//! to transcode it, because that is what the choice actually buys. Carrying a preset out is
//! a separate concern: the presets are a friendly surface over the community-maintained
//! profiles ([TRaSH](https://trash-guides.info) via the quality sync tool), so the hard
//! part stays maintained upstream and this only translates the question. Nothing here
//! reaches a service or a disk; it is the pure model the surface and the quality sync
//! writer are built on.

pub(crate) mod run;

pub use run::{offered, CHANGES};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::audio::Format;

pub use lemonfiber_ports::media::preset::{Consequence, Preset};

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
