//! What kind of thing lemonfiber is handling — the words the boundary needs to name it.
//!
//! Which \*arr an item belongs to, and how good an operator wants their music. Both are
//! choices made well above this, and both have to cross the boundary: a port that could not
//! say which service it is asking about, or what quality to apply, would push the naming
//! into every caller.
//!
//! The definitions live here and are re-exported by the modules that own the *decisions* —
//! a port cannot reach up into the layer it serves, and the layer above is free to keep
//! calling them by their old names.

/// The two services whose quality an operator chooses by resolution, and so the
/// two the quality model speaks to. Music and books have a different axis and
/// are not resolution presets, so they are not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Sonarr — television.
    Sonarr,
    /// Radarr — film.
    Radarr,
}

impl Kind {
    /// Both services, in the order `recyclarr.yml` lists them.
    pub const ALL: [Self; 2] = [Self::Sonarr, Self::Radarr];

    /// The top-level `recyclarr.yml` section this service is configured under.
    #[must_use]
    pub const fn section(self) -> &'static str {
        match self {
            Self::Sonarr => "sonarr",
            Self::Radarr => "radarr",
        }
    }

    /// The media type in a [`Selection`] this service draws its preset from, so a
    /// per-type choice — Maximum for film, Balanced for television — reaches the
    /// right service.
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Sonarr => "tv",
            Self::Radarr => "movies",
        }
    }

    /// The service whose section a top-level `recyclarr.yml` key names — also the
    /// service a compose id such as `sonarr` names, since the two share the word —
    /// or `None` for any other key, so an operator's own additions are left alone.
    #[must_use]
    pub fn for_section(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.section() == key)
    }

    /// The Servarr command that re-searches existing content for a better release
    /// meeting the current quality profile — the "upgrade what is already here"
    /// action. Named per service, verified against each app's command set.
    #[must_use]
    pub const fn upgrade_command(self) -> &'static str {
        match self {
            Self::Sonarr => "CutoffUnmetEpisodeSearch",
            Self::Radarr => "CutoffUnmetMoviesSearch",
        }
    }

    /// The query parameter naming what a manual release search is for — an episode for
    /// television, a movie for film. A wanted item's own id fills it, so a search asks
    /// the indexers for exactly what the operator is missing.
    #[must_use]
    pub const fn release_id_param(self) -> &'static str {
        match self {
            Self::Sonarr => "episodeId",
            Self::Radarr => "movieId",
        }
    }

    /// The API path segment listing the service's library — the series it tracks, or the
    /// films — searched by a human term to find an item to trace.
    #[must_use]
    pub const fn library_endpoint(self) -> &'static str {
        match self {
            Self::Sonarr => "series",
            Self::Radarr => "movie",
        }
    }

    /// The history query parameter that filters events to one library item, so a trace
    /// reads only what happened to the item asked about.
    #[must_use]
    pub const fn history_filter(self) -> &'static str {
        match self {
            Self::Sonarr => "seriesIds",
            Self::Radarr => "movieIds",
        }
    }

    /// The plain word for what this service files, as a household would say it — the
    /// noun a request is named by where its title could not be found.
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Sonarr => "series",
            Self::Radarr => "film",
        }
    }

    /// The API path segment listing the parts a library item is made of — the episodes of
    /// a series — or `None` for a service whose items have no parts. A film is the whole
    /// item, so there is nothing to aggregate and nothing to ask for.
    #[must_use]
    pub const fn parts_endpoint(self) -> Option<&'static str> {
        match self {
            Self::Sonarr => Some("episode"),
            Self::Radarr => None,
        }
    }

    /// The external catalogue this service files by — the identifier an add is made
    /// with. The two services use different catalogues, and a field one does not know is
    /// a field it refuses the whole request over.
    #[must_use]
    pub const fn reference_field(self) -> &'static str {
        match self {
            Self::Sonarr => "tvdbId",
            Self::Radarr => "tmdbId",
        }
    }

    /// The add option that tells the service to go and look for what it has just taken
    /// on, rather than filing it and waiting for its next scheduled sweep — which is the
    /// difference between a walkthrough and a bookmark.
    #[must_use]
    pub const fn search_option(self) -> &'static str {
        match self {
            Self::Sonarr => "searchForMissingEpisodes",
            Self::Radarr => "searchForMovie",
        }
    }

    /// The query parameter that narrows the parts listing to one library item.
    #[must_use]
    pub const fn parts_filter(self) -> &'static str {
        match self {
            Self::Sonarr => "seriesId",
            Self::Radarr => "movieId",
        }
    }
}

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
