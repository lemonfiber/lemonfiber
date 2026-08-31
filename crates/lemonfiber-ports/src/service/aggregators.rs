//! The indexer sources a service pulls from, where it pulls rather than being pushed to.
//!
//! Most of the stack is told about its indexers by the aggregator itself, which
//! registers into each \*arr through its own application sync. The book \*arr is not
//! one of those: it keeps its own list of aggregators and reads from them, so the
//! connection is made by telling it where one is rather than by telling the aggregator
//! about it.
//!
//! A port of its own for the same reason the subtitle finder has one — this is a
//! service told about another, rather than one of the family the shared client covers.

use super::Failure;
use async_trait::async_trait;

/// An aggregator, as the service pulling from it needs to be told about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggregator {
    /// What it is called where somebody reads the list.
    pub name: String,
    /// Where it is reached — a name on the stack's own network, since the service
    /// doing the reading is a container beside it.
    pub url: String,
    /// The aggregator's own key, which is what lets the list be read at all.
    pub key: String,
}

/// An aggregator the service already holds.
///
/// Read back so one already known is left alone rather than added twice, and so a
/// registration can be confirmed by reading it. **Whether it holds a key is part of
/// this**: the service accepts a registration whose key it did not understand and
/// answers success, so an entry without one is the failure this reads for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownAggregator {
    /// The identifier the service gave it.
    pub id: String,
    /// Where it says the aggregator is.
    pub url: String,
    /// Whether it holds a key for it. The key itself is not read back — a credential
    /// is not something to carry around to decide whether to write one.
    pub keyed: bool,
}

/// A service that pulls its indexers from an aggregator it is told about.
#[async_trait]
pub trait Aggregators: Send + Sync {
    /// The aggregators it already reads from.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn aggregators(&self) -> Result<Vec<KnownAggregator>, Failure>;

    /// Tell it about an aggregator, with the key that lets it read one.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn add_aggregator(&self, aggregator: &Aggregator) -> Result<(), Failure>;
}
