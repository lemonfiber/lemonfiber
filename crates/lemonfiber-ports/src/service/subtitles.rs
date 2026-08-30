//! The services a subtitle finder watches for something to subtitle.
//!
//! It does not discover them. Until it is told, it runs with nothing to do — the
//! household gets subtitles for nothing, which looks exactly like a household whose
//! releases happen not to have any.

use super::Failure;
use async_trait::async_trait;

/// Which \*arr a subtitle finder is being pointed at.
///
/// Only the two that file what carries subtitles. Music and books have none to find,
/// so there is no variant for them rather than a variant that is never used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subtitled {
    /// Sonarr — television.
    Sonarr,
    /// Radarr — film.
    Radarr,
}

impl Subtitled {
    /// The name the subtitle finder files this \*arr's settings under.
    #[must_use]
    pub const fn section(self) -> &'static str {
        match self {
            Self::Sonarr => "sonarr",
            Self::Radarr => "radarr",
        }
    }
}

/// An \*arr, as a subtitle finder needs to be told about it.
///
/// The host is a name on the stack's own network rather than an address: the finder
/// is a container beside them, so `127.0.0.1` there is the finder itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
    /// Which \*arr it is, selecting the settings it is filed under.
    pub which: Subtitled,
    /// The host the finder reaches it on.
    pub host: String,
    /// The port it listens on.
    pub port: u16,
    /// The \*arr's own key, which is what lets the finder read what it has.
    pub api_key: String,
}

/// What a subtitle finder already holds for one \*arr.
///
/// Read back so one already pointed at an \*arr is left alone rather than written
/// again, and so a write can be confirmed by reading it.
///
/// **The base path is not part of this**, deliberately: it is normalised on the way
/// in — `/` is stored as empty — so comparing it reports a difference nobody made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watching {
    /// Whether the finder is set to use this \*arr at all. Off is the default, and
    /// an address written with this left off is an address nothing reads.
    pub enabled: bool,
    /// The host it holds.
    pub host: String,
    /// The port it holds.
    pub port: u16,
    /// Whether it holds a key. The key itself is not read back for comparison — a
    /// credential is not something to carry around to decide whether to write one.
    pub keyed: bool,
}

/// A subtitle finder, told which \*arrs to watch.
///
/// A port of its own rather than a method on the shared \*arr shape, because this is
/// the one service that is told about the others rather than being one of them —
/// the same reason the indexer's application sync is its own port.
#[async_trait]
pub trait Subtitles: Send + Sync {
    /// What the finder currently holds for `which`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn watching(&self, which: Subtitled) -> Result<Watching, Failure>;

    /// Point the finder at an \*arr, and switch it on.
    ///
    /// Both at once, because either alone is nothing: an address the finder is not
    /// set to use is never read, and switching it on with no address gives it
    /// somewhere unreachable to look.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn watch(&self, watched: &Watched) -> Result<(), Failure>;
}
