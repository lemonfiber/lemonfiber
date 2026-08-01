//! Audio quality in plain language, for media that has no resolution.
//!
//! A film or a series has a resolution — 1080p, 4K — and [`quality::Preset`](crate::quality)
//! is the operator's answer to *how good should this look*. Music has no resolution;
//! its quality axis is the audio format itself: a small lossy file, a lossless copy
//! of the CD, or a hi-res studio master. So the same friendly question — *how good,
//! and how much disk* — needs a second, format-shaped answer here rather than a
//! resolution one bent to fit.
//!
//! This is that answer as three formats, each stating in plain terms what it means
//! and what it costs. Carrying it out is a separate concern: unlike the resolution
//! presets there is no community profile to lean on (Recyclarr configures only
//! Sonarr and Radarr), so the format maps to Lidarr's own quality profile, applied
//! through its API. Nothing here reaches a service or a disk; it is the pure model
//! that surface and the Lidarr writer are built on.

use serde::{Deserialize, Serialize};

/// How good the operator wants their music to sound, and how much disk they will
/// spend on it — chosen as a format preference, since music has no resolution to
/// choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// Small lossy files that sound great on phones, earbuds, and in the car:
    /// MP3/AAC at 320 kbps.
    Compact,
    /// A perfect copy of the CD, every bit kept: lossless FLAC or ALAC. The one
    /// chosen when the operator expresses no preference — the reason to keep a
    /// music library rather than stream it.
    Lossless,
    /// Studio-master quality where it exists, for a good hi-fi: 24-bit lossless.
    HiRes,
}

/// What choosing a format practically costs: the format it targets, roughly how much
/// disk an hour of it takes, and what to know about playing or finding it — the
/// three things the choice actually decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Consequence {
    /// The audio format the choice targets, in plain terms.
    pub format: &'static str,
    /// Roughly how much disk an hour of music at this format takes.
    pub size_per_hour: &'static str,
    /// What to know about playing it or finding it — the practical caveat beyond
    /// size, since a format the operator's devices cannot play, or that no release
    /// carries, is a cost too.
    pub note: &'static str,
}

impl Format {
    /// Every format, in the order they are offered — least to most demanding.
    pub const ALL: [Self; 3] = [Self::Compact, Self::Lossless, Self::HiRes];

    /// The format chosen when the operator expresses no preference: a lossless copy
    /// of the CD, the reason to keep a library rather than stream it.
    #[must_use]
    pub const fn default_format() -> Self {
        Self::Lossless
    }

    /// The plain-language name an operator selects the format by, and that it is
    /// stored under.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Lossless => "lossless",
            Self::HiRes => "hi-res",
        }
    }

    /// The format an operator named, or `None` where the name is not one of the three
    /// — so a mistyped choice is refused rather than guessed.
    #[must_use]
    pub fn from_label(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.label() == name)
    }

    /// What the format means, in the operator's terms rather than the tool's.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::Compact => "Small files that sound great on phones and in the car.",
            Self::Lossless => "A perfect copy of the CD — every bit of the recording kept.",
            Self::HiRes => "Studio-master quality where it exists, for a good hi-fi.",
        }
    }

    /// What the format practically costs — the format targeted, size per hour, and the
    /// caveat worth knowing — so the choice is made knowing its consequence.
    #[must_use]
    pub const fn consequence(self) -> Consequence {
        match self {
            Self::Compact => Consequence {
                format: "MP3 or AAC at 320 kbps",
                size_per_hour: "about 130–150 MB per hour",
                note: "plays on anything and streams easily over a slow connection",
            },
            Self::Lossless => Consequence {
                format: "FLAC or ALAC, CD quality (16-bit)",
                size_per_hour: "about 300–600 MB per hour",
                note: "identical to the CD; a remote device may transcode it to stream",
            },
            Self::HiRes => Consequence {
                format: "FLAC 24-bit, the studio master where it exists",
                size_per_hour: "about 1.5–2.5 GB per hour",
                note: "audiophile quality, but often unavailable and several times the size",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Format;

    #[test]
    fn the_default_is_a_lossless_copy_of_the_cd() {
        // The reason to keep a library rather than stream it — the middle of the three,
        // chosen when the operator says nothing.
        assert_eq!(Format::default_format(), Format::Lossless);
    }

    #[test]
    fn every_format_round_trips_through_its_plain_label() {
        for format in Format::ALL {
            assert_eq!(Format::from_label(format.label()), Some(format));
        }
    }

    #[test]
    fn a_name_that_is_not_a_format_is_refused_rather_than_guessed() {
        assert_eq!(Format::from_label("lossy"), None);
        assert_eq!(Format::from_label(""), None);
    }

    #[test]
    fn a_label_is_plain_language_with_no_format_or_bitrate_jargon() {
        // The operator selects by "how good", never by a codec or a bitrate — the same
        // plain-language contract the resolution presets keep.
        for format in Format::ALL {
            let label = format.label();
            assert!(!label.is_empty());
            assert!(!label.contains("FLAC"));
            assert!(!label.contains("MP3"));
            assert!(!label.contains("kbps"));
            assert!(!label.contains("bit"));
        }
    }

    #[test]
    fn each_format_states_a_meaning_a_size_and_a_caveat() {
        for format in Format::ALL {
            assert!(!format.means().is_empty());
            let consequence = format.consequence();
            assert!(!consequence.format.is_empty());
            assert!(
                consequence.size_per_hour.contains("MB")
                    || consequence.size_per_hour.contains("GB"),
                "{} names no size",
                consequence.size_per_hour
            );
            assert!(!consequence.note.is_empty());
        }
    }

    #[test]
    fn the_formats_are_offered_least_to_most_demanding() {
        // Compact, then lossless, then hi-res — the order the surface presents, and the
        // order that makes "most demanding" meaningful for a future projection.
        assert_eq!(
            Format::ALL,
            [Format::Compact, Format::Lossless, Format::HiRes]
        );
    }

    #[test]
    fn a_format_serialises_under_its_plain_label() {
        // The stored form is the label the operator chose, not an integer or a codec —
        // the choice file reads as the question it answered.
        let json = serde_json::to_string(&Format::HiRes).unwrap_or_default();
        assert_eq!(json, r#""hi-res""#);
        let back: Option<Format> = serde_json::from_str(&json).ok();
        assert_eq!(back, Some(Format::HiRes));
    }
}
