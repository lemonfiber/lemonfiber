//! The two download capabilities: what a Usenet client and a torrent client are asked.
//!
//! Two contracts rather than one with optional parts, because they are two capabilities
//! the vocabulary names apart and asks for apart. They share what any download client
//! answers, and each carries the one thing only its kind has: a Usenet client's provider
//! accounts, a torrent client's seeding.

use lemonfiber_ports::service::{
    Download, Fetching, Metering, Moved, Pulling, Rates, Seeded, Seeding, Throttled, Throttling,
    Transfers, UsenetAccount, UsenetAccounts, Wanted,
};

crate::contract! {
    /// `download.usenet`: a Usenet client.
    pub mod usenet = "download.usenet" @ 1 {
        impl Transfers {
            /// What it is downloading now.
            fn transfers() -> Vec<Download>;
        }
        impl Fetching {
            /// Whether it is fetching.
            fn pulling() -> Pulling;
            /// Stop fetching.
            fn stop() -> Pulling;
            /// Fetch again.
            fn resume() -> Pulling;
        }
        impl Throttling {
            /// The rates it is held to now.
            fn throttled() -> Throttled;
            /// Hold it to these rates.
            fn restrain(refer wanted: &Wanted as Wanted) -> Throttled;
            /// How fast it is moving now.
            fn moving() -> Rates;
        }
        impl Metering {
            /// What it moved in one month, `YYYY-MM`.
            fn moved(str month: &str) -> Moved;
        }
        impl UsenetAccounts {
            /// The provider accounts it downloads through.
            fn accounts() -> Vec<UsenetAccount>;
        }
    }
}

crate::contract! {
    /// `download.torrent`: a torrent client.
    pub mod torrent = "download.torrent" @ 1 {
        impl Transfers {
            /// What it is downloading now.
            fn transfers() -> Vec<Download>;
        }
        impl Fetching {
            /// Whether it is fetching.
            fn pulling() -> Pulling;
            /// Stop fetching.
            fn stop() -> Pulling;
            /// Fetch again.
            fn resume() -> Pulling;
        }
        impl Throttling {
            /// The rates it is held to now.
            fn throttled() -> Throttled;
            /// Hold it to these rates.
            fn restrain(refer wanted: &Wanted as Wanted) -> Throttled;
            /// How fast it is moving now.
            fn moving() -> Rates;
        }
        impl Metering {
            /// What it moved in one month, `YYYY-MM`.
            fn moved(str month: &str) -> Moved;
        }
        impl Seeding {
            /// What it is still seeding.
            fn seeding() -> Vec<Seeded>;
            /// Let one completed download go, with its files.
            fn stop_seeding(str name: &str) -> ();
        }
    }
}
