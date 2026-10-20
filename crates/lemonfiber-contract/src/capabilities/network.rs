//! `network.egress-guard`: what the egress guard is asked, the tunnel every download
//! inside it leaves through.
//!
//! Whether its tunnel is up, and the port its provider forwards now, which the core
//! hands on to every torrent client inside it.

use lemonfiber_ports::service::Tunnelling;

crate::contract! {
    /// `network.egress-guard`: the egress guard.
    pub mod egress = "network.egress-guard" @ 1 {
        impl Tunnelling {
            /// Whether its tunnel is up.
            fn up() -> bool where asks = Asked {};
            /// The port its provider forwards now, or none.
            fn forwarded() -> Option<u16> where asks = Asked {};
        }
    }
}
