//! `proxy.front`: what the proxy in front of the household's services is asked.
//!
//! It is handed its whole routing table, read back, and asked to read what it was handed.

use lemonfiber_ports::service::{Applied, Proxying, Route, Routing, Tls};

/// The household name the conformance cases route.
const NAME: &str = "conformance.home.local";

/// The host the conformance cases route it to.
const UPSTREAM: &str = "conformance-upstream";

/// The port the conformance cases route it to.
const UPSTREAM_PORT: u16 = 80;

crate::contract! {
    /// `proxy.front`: the household's proxy.
    pub mod front = "proxy.front" @ 1 {
        impl Proxying {
            /// Serve a routing table in place of the one it serves.
            fn apply(refer table: &Routing as Routing) -> Applied where asks = Asked {
                table: Routing {
                    tls: Tls::Automatic,
                    routes: vec![Route {
                        name: NAME.to_owned(),
                        host: UPSTREAM.to_owned(),
                        port: UPSTREAM_PORT,
                    }],
                },
            };
            /// The table it serves.
            fn routing() -> Routing where asks = Asked {};
            /// Read the table applied.
            fn reload() -> Applied where asks = Asked {};
        }
    }
}
