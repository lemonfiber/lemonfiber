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

/// The rate, in bytes a second, the conformance cases hold a client to while active.
const RATE: u64 = 1_048_576;

/// The month the conformance cases ask what moved in.
const MONTH: &str = "2026-01";

/// A torrent no client holds.
const TORRENT: &str = "conformance-torrent";

/// What the conformance cases ask a client to be held to.
fn wanted() -> Wanted {
    Wanted {
        active: Rates {
            down: Some(RATE),
            up: Some(RATE),
        },
        quiet: Rates {
            down: None,
            up: None,
        },
        window: None,
    }
}

crate::contract! {
    /// `download.usenet`: a Usenet client.
    pub mod usenet = "download.usenet" @ 1 {
        impl Transfers {
            /// What it is downloading now.
            fn transfers() -> Vec<Download> where asks = Asked {};
        }
        impl Fetching {
            /// Whether it is fetching.
            fn pulling() -> Pulling where asks = Asked {};
            /// Stop fetching.
            fn stop() -> Pulling where asks = Asked {};
            /// Fetch again.
            fn resume() -> Pulling where asks = Asked {};
        }
        impl Throttling {
            /// The rates it is held to now.
            fn throttled() -> Throttled where asks = Asked {};
            /// Hold it to these rates.
            fn restrain(refer wanted: &Wanted as Wanted) -> Throttled where asks = Asked { wanted: wanted() };
            /// How fast it is moving now.
            fn moving() -> Rates where asks = Asked {};
        }
        impl Metering {
            /// What it moved in one month, `YYYY-MM`.
            fn moved(str month: &str) -> Moved where asks = Asked { month: MONTH.to_owned() };
        }
        impl UsenetAccounts {
            /// The provider accounts it downloads through.
            fn accounts() -> Vec<UsenetAccount> where asks = Asked {};
        }
    }
}

crate::contract! {
    /// `download.torrent`: a torrent client.
    pub mod torrent = "download.torrent" @ 1 {
        impl Transfers {
            /// What it is downloading now.
            fn transfers() -> Vec<Download> where asks = Asked {};
        }
        impl Fetching {
            /// Whether it is fetching.
            fn pulling() -> Pulling where asks = Asked {};
            /// Stop fetching.
            fn stop() -> Pulling where asks = Asked {};
            /// Fetch again.
            fn resume() -> Pulling where asks = Asked {};
        }
        impl Throttling {
            /// The rates it is held to now.
            fn throttled() -> Throttled where asks = Asked {};
            /// Hold it to these rates.
            fn restrain(refer wanted: &Wanted as Wanted) -> Throttled where asks = Asked { wanted: wanted() };
            /// How fast it is moving now.
            fn moving() -> Rates where asks = Asked {};
        }
        impl Metering {
            /// What it moved in one month, `YYYY-MM`.
            fn moved(str month: &str) -> Moved where asks = Asked { month: MONTH.to_owned() };
        }
        impl Seeding {
            /// What it is still seeding.
            fn seeding() -> Vec<Seeded> where asks = Asked {};
            /// Let one completed download go, with its files.
            fn stop_seeding(str name: &str) -> () where asks = Asked { name: TORRENT.to_owned() };
        }
    }
}
