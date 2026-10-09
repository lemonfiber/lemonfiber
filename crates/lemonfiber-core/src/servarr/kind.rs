//! What each kind of video is called on the Servarr wire.
//!
//! The media kinds the ports name are the household's words, television and film. A
//! Servarr app speaks of them in its own: a config section, a command, a query parameter,
//! a path. Those words are this adapter's to know and nobody else's, so they sit here,
//! beside the client that sends them, as methods on the neutral kind.

use crate::ports::media::Kind;

/// The Servarr spelling of each kind of video.
pub(crate) trait Shape: Sized {
    /// The top-level `recyclarr.yml` section this service is configured under.
    #[must_use]
    fn section(self) -> &'static str;

    /// The service whose section a top-level `recyclarr.yml` key names — also the
    /// service a compose id such as `sonarr` names, since the two share the word —
    /// or `None` for any other key, so an operator's own additions are left alone.
    #[must_use]
    fn for_section(key: &str) -> Option<Kind>;

    /// The Servarr command that re-searches existing content for a better release
    /// meeting the current quality profile — the "upgrade what is already here"
    /// action. Named per service, verified against each app's command set.
    #[must_use]
    fn upgrade_command(self) -> &'static str;

    /// The query parameter naming what a manual release search is for — an episode for
    /// television, a movie for film. A wanted item's own id fills it, so a search asks
    /// the indexers for exactly what the operator is missing.
    #[must_use]
    fn release_id_param(self) -> &'static str;

    /// The API path segment listing the service's library — the series it tracks, or the
    /// films — searched by a human term to find an item to trace.
    #[must_use]
    fn library_endpoint(self) -> &'static str;

    /// The history query parameter that filters events to one library item, so a trace
    /// reads only what happened to the item asked about.
    #[must_use]
    fn history_filter(self) -> &'static str;

    /// The API path segment listing the parts a library item is made of — the episodes of
    /// a series — or `None` for a service whose items have no parts. A film is the whole
    /// item, so there is nothing to aggregate and nothing to ask for.
    #[must_use]
    fn parts_endpoint(self) -> Option<&'static str>;

    /// The external catalogue this service files by — the identifier an add is made
    /// with. The two services use different catalogues, and a field one does not know is
    /// a field it refuses the whole request over.
    #[must_use]
    fn reference_field(self) -> &'static str;

    /// The add option that tells the service to go and look for what it has just taken
    /// on, rather than filing it and waiting for its next scheduled sweep — which is the
    /// difference between a walkthrough and a bookmark.
    #[must_use]
    fn search_option(self) -> &'static str;

    /// The query parameter that narrows the parts listing to one library item.
    #[must_use]
    fn parts_filter(self) -> &'static str;
}

impl Shape for Kind {
    fn section(self) -> &'static str {
        match self {
            Kind::Tv => "sonarr",
            Kind::Movies => "radarr",
        }
    }

    fn for_section(key: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.section() == key)
    }

    fn upgrade_command(self) -> &'static str {
        match self {
            Kind::Tv => "CutoffUnmetEpisodeSearch",
            Kind::Movies => "CutoffUnmetMoviesSearch",
        }
    }

    fn release_id_param(self) -> &'static str {
        match self {
            Kind::Tv => "episodeId",
            Kind::Movies => "movieId",
        }
    }

    fn library_endpoint(self) -> &'static str {
        match self {
            Kind::Tv => "series",
            Kind::Movies => "movie",
        }
    }

    fn history_filter(self) -> &'static str {
        match self {
            Kind::Tv => "seriesIds",
            Kind::Movies => "movieIds",
        }
    }

    fn parts_endpoint(self) -> Option<&'static str> {
        match self {
            Kind::Tv => Some("episode"),
            Kind::Movies => None,
        }
    }

    fn reference_field(self) -> &'static str {
        match self {
            Kind::Tv => "tvdbId",
            Kind::Movies => "tmdbId",
        }
    }

    fn search_option(self) -> &'static str {
        match self {
            Kind::Tv => "searchForMissingEpisodes",
            Kind::Movies => "searchForMovie",
        }
    }

    fn parts_filter(self) -> &'static str {
        match self {
            Kind::Tv => "seriesId",
            Kind::Movies => "movieId",
        }
    }
}
