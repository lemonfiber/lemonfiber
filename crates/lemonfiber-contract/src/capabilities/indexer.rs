//! `indexer.search`: what an indexer is asked, the service that searches release sources
//! on behalf of the curators it is told about.
//!
//! The applications it searches for, each named by the media it files; the sources it
//! searches and how hard they are used; and the aggregators a curator that pulls rather
//! than being pushed to reads from it.

use std::time::SystemTime;

use lemonfiber_ports::service::{
    Aggregator, Aggregators, AppSync, Application, IndexerUse, Indexers, KnownAggregator,
    RegisteredApplication,
};

crate::contract! {
    /// `indexer.search`: an indexer.
    pub mod search = "indexer.search" @ 1 {
        impl AppSync {
            /// Tell it about an application to search for.
            fn register_application(refer application: &Application as Application) -> ();
            /// The applications it already searches for.
            fn applications() -> Vec<RegisteredApplication>;
            /// Whether it reaches an application it holds.
            fn test_application(
                refer held: &RegisteredApplication as RegisteredApplication,
            ) -> ();
            /// Give an application it holds a new key.
            fn rekey_application(
                refer held: &RegisteredApplication as RegisteredApplication,
                str key: &str,
            ) -> ();
        }
        impl Indexers {
            /// Every source it searches, with how hard each is used as of `now`.
            fn indexers(value now: SystemTime) -> Vec<IndexerUse>;
        }
        impl Aggregators {
            /// The aggregators it reads from.
            fn aggregators() -> Vec<KnownAggregator>;
            /// Read from one more aggregator.
            fn add_aggregator(refer aggregator: &Aggregator as Aggregator) -> ();
        }
    }
}
