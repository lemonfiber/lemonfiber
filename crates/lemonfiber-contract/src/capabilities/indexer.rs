//! `indexer.search`: what an indexer is asked, the service that searches release sources
//! on behalf of the curators it is told about.
//!
//! The applications it searches for, each named by the media it files; the sources it
//! searches and how hard they are used; and the aggregators a curator that pulls rather
//! than being pushed to reads from it.

use std::time::{Duration, SystemTime};

use lemonfiber_ports::service::{
    Aggregator, Aggregators, AppSync, Application, ApplicationKind, IndexerUse, Indexers,
    KnownAggregator, RegisteredApplication,
};

use crate::conformance::Placeholder;

/// The name the conformance cases register an application and an aggregator under.
const NAMED: &str = "conformance";

/// Where the conformance cases say the indexer is reached from an application.
const INDEXER_URL: &str = "http://conformance-indexer";

/// Where the conformance cases say an application is reached.
const APPLICATION_URL: &str = "http://conformance-curator";

/// Where the conformance cases say an aggregator is reached.
const AGGREGATOR_URL: &str = "http://conformance-aggregator";

/// The moment the conformance cases ask about use at, in seconds after the epoch.
const NOW: u64 = 1_767_225_600;

/// The application this contract's `applications` case answered first.
fn registered() -> RegisteredApplication {
    let earlier = |pointer: &str| {
        Placeholder::answer(search::CAPABILITY, search::MAJOR, "applications", pointer).written()
    };
    RegisteredApplication {
        id: earlier("/0/id"),
        base_url: earlier("/0/base_url"),
    }
}

crate::contract! {
    /// `indexer.search`: an indexer.
    pub mod search = "indexer.search" @ 1 {
        impl AppSync {
            /// Tell it about an application to search for.
            fn register_application(refer application: &Application as Application) -> () where asks = Asked {
                application: Application {
                    name: NAMED.to_owned(),
                    kind: ApplicationKind::Tv,
                    indexer_url: INDEXER_URL.to_owned(),
                    base_url: APPLICATION_URL.to_owned(),
                    api_key: Placeholder::Minted.written(),
                },
            };
            /// The applications it already searches for.
            fn applications() -> Vec<RegisteredApplication> where asks = Asked {};
            /// Whether it reaches an application it holds.
            fn test_application(
                refer held: &RegisteredApplication as RegisteredApplication,
            ) -> () where asks = Asked { held: registered() };
            /// Give an application it holds a new key.
            fn rekey_application(
                refer held: &RegisteredApplication as RegisteredApplication,
                str key: &str,
            ) -> () where asks = Asked { held: registered(), key: Placeholder::Minted.written() };
        }
        impl Indexers {
            /// Every source it searches, with how hard each is used as of `now`.
            fn indexers(value now: SystemTime) -> Vec<IndexerUse> where asks = Asked { now: SystemTime::UNIX_EPOCH + Duration::from_secs(NOW) };
        }
        impl Aggregators {
            /// The aggregators it reads from.
            fn aggregators() -> Vec<KnownAggregator> where asks = Asked {};
            /// Read from one more aggregator.
            fn add_aggregator(refer aggregator: &Aggregator as Aggregator) -> () where asks = Asked {
                aggregator: Aggregator {
                    name: NAMED.to_owned(),
                    url: AGGREGATOR_URL.to_owned(),
                    key: Placeholder::Minted.written(),
                },
            };
        }
    }
}
