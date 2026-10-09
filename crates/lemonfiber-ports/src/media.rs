//! What kind of thing lemonfiber is handling — the words the boundary needs to name it.
//!
//! Which kind of video an item is, and how good an operator wants their music. Both are
//! choices made well above this, and both have to cross the boundary: a port that could not
//! say which service it is asking about, or what quality to apply, would push the naming
//! into every caller.
//!
//! The definitions live here and are re-exported by the modules that own the *decisions* —
//! a port cannot reach up into the layer it serves, and the layer above is free to keep
//! calling them by their old names.

use serde::{Deserialize, Serialize};

/// The two kinds of video a household files by resolution, and so the two the quality
/// model speaks to. Music and books have a different axis and are not resolution
/// presets, so they are not here.
///
/// Named for what is filed, never for the service that files it: whatever curates
/// television is asked about `Tv`. Read as the media type a service declares, and
/// accepting the old service-named spellings where a record still holds one.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Television: series, seasons and episodes.
    #[serde(alias = "sonarr")]
    Tv,
    /// Film.
    #[serde(alias = "radarr")]
    Movies,
}

impl Kind {
    /// Both, television first.
    pub const ALL: [Self; 2] = [Self::Tv, Self::Movies];

    /// The media type a service declares for this kind, and a [`Selection`] draws its
    /// preset by, so a per-type choice reaches the right curator.
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Tv => "tv",
            Self::Movies => "movies",
        }
    }

    /// The kind a declared media type names, or `None` for one filed by some other axis.
    #[must_use]
    pub fn for_media_type(media_type: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.media_type() == media_type)
    }

    /// The kind of video a service files, read from the media types it declares, or
    /// `None` for a service that files no video.
    ///
    /// Television before film whatever order they are declared in: a service filing
    /// both is filed as one, and which one has to be the same every run.
    #[must_use]
    pub fn of_declared(media_types: &[String]) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| {
            media_types
                .iter()
                .any(|media| media.as_str() == kind.media_type())
        })
    }

    /// The plain word for what is filed, as a household would say it — the noun a
    /// request is named by where its title could not be found.
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Tv => "series",
            Self::Movies => "film",
        }
    }
}

/// How good the operator wants their music to sound, and how much disk they will
/// spend on it — chosen as a format preference, since music has no resolution to
/// choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
mod tests;
