//! Carrying an operator's own records from one stack onto another.
//!
//! The vocabulary for that errand kept in one place: which kinds can cross, what one
//! of them looks like in flight, and the port that reads and re-creates them.
//! Everything else in this module tree is lemonfiber wiring services to each other,
//! which it decides; this moves across what somebody else already decided, and decides
//! nothing about it.

use async_trait::async_trait;

use super::Failure;

/// A kind of record a service holds, for carrying a setup across.
///
/// Four, because four are what an operator would miss: where things are found, and the
/// three libraries the *arrs keep. Each is a path under the same API and behaves the
/// same way, which is why one port serves all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Record {
    /// Where releases are searched for.
    Indexer,
    /// Television the service is following.
    Series,
    /// Films the service is following.
    Film,
    /// Music the service is following.
    Artist,
}

impl Record {
    /// The path this kind lives at, under the service's versioned API.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Indexer => "/indexer",
            Self::Series => "/series",
            Self::Film => "/movie",
            Self::Artist => "/artist",
        }
    }

    /// What a person calls this kind, for a line about what was carried.
    #[must_use]
    pub const fn plural(self) -> &'static str {
        match self {
            Self::Indexer => "indexers",
            Self::Series => "series",
            Self::Film => "films",
            Self::Artist => "artists",
        }
    }
}

/// One record a service holds, as it holds it.
///
/// The profile and the folder are carried **by name and by path**, never by the ids the
/// service gave them. Two stacks number their own profiles and folders independently, so
/// a record copied with its ids intact would point at whatever happened to be third on
/// the other machine — which is the quiet way an import ruins a library rather than
/// failing to copy it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    /// What it is called, which is how a copy already here is recognised.
    pub name: String,
    /// The quality profile it follows, by name.
    pub profile: Option<String>,
    /// The root folder it sits under, by path.
    pub folder: Option<String>,
    /// Everything else the service said about it, kept as it was said.
    pub rest: String,
}

/// Reading what a service holds and re-creating it on another.
///
/// Its own port because it is neither provisioning nor a read of state: it is the one
/// thing that copies an operator's own records from a stack lemonfiber did not build
/// into one it did.
#[async_trait]
pub trait Carrying: Send + Sync {
    /// Every record of a kind this service holds.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or answers unusably.
    async fn records(&self, kind: Record) -> Result<Vec<Carried>, Failure>;

    /// Re-create a record here, pointing it at this service's own profile and folder.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service refuses it, or names a profile or folder
    /// this service does not have.
    async fn carry(&self, kind: Record, item: &Carried) -> Result<(), Failure>;
}

#[cfg(test)]
mod tests {
    use super::Record;

    /// Four kinds, four paths. The three libraries are three different words for the
    /// same idea across the three services, and getting one wrong would carry television
    /// into the films.
    #[test]
    fn each_kind_of_record_names_its_own_path_and_its_own_word() {
        assert_eq!(Record::Indexer.path(), "/indexer");
        assert_eq!(Record::Series.path(), "/series");
        assert_eq!(Record::Film.path(), "/movie");
        assert_eq!(Record::Artist.path(), "/artist");

        assert_eq!(Record::Indexer.plural(), "indexers");
        assert_eq!(Record::Series.plural(), "series");
        assert_eq!(Record::Film.plural(), "films");
        assert_eq!(Record::Artist.plural(), "artists");
    }
}
