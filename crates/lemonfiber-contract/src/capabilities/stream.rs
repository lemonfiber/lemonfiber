//! `stream.guard`: what the guard in front of the media server is asked.
//!
//! It is handed one envelope, the media server's declaration with the one proxy it
//! believes and where it asks about a live-stream pass, read back whole, and asked for
//! the certificate it presents.

use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Addressee, Answers, BytePath, Declared, Guarded, Guarding, Held, LivePath, Locations, Naming,
    Ports, Presentation, Presented, Question, Statuses, Token,
};

use super::PROXY;

/// The shape of an item's id, and of a token, in the conformance cases' declaration.
const HEX: &str = "^[0-9a-f]{32}$";

/// The port the conformance cases' media server listens on.
const UPSTREAM_PORT: u16 = 80;

/// Where the conformance cases' guard reaches the media server.
const UPSTREAM: &str = "http://conformance-upstream";

/// Where the conformance cases' guard asks about a pass.
const PASS: &str = "http://conformance-pass/pass";

/// A declaration of one byte path, held to everything a guard enforces.
fn declared() -> Declared {
    Declared {
        bytes: vec![BytePath {
            pattern: "^/items/(?P<item>[^/]+)/bytes$".to_owned(),
            item: HEX.to_owned(),
            exempt: vec!["^/items/playing/bytes$".to_owned()],
            naming: vec![
                Naming {
                    parameter: "other".to_owned(),
                    held: Held::Refused,
                },
                Naming {
                    parameter: "source".to_owned(),
                    held: Held::Asked {
                        shape: HEX.to_owned(),
                    },
                },
            ],
        }],
        live: vec![LivePath {
            pattern: "^/live/(?P<stream>[^/]+)/".to_owned(),
            stream: HEX.to_owned(),
        }],
        refused: vec!["^/everything/picture$".to_owned()],
        question: Some(Question {
            method: Method::Get,
            path: "/items/{item}".to_owned(),
            to: Addressee::Upstream,
            answers: Answers {
                may: vec![Statuses { from: 200, to: 299 }],
                may_not: vec![Statuses::only(404)],
                no_session: vec![Statuses::only(401)],
                not_from_there: vec![Statuses::only(403)],
            },
        }),
        token: Token {
            presented: Presentation::AsIs,
            shape: HEX.to_owned(),
            parameters: Vec::new(),
        },
        locations: Locations {
            stream: "{base}/items/{item}/bytes".to_owned(),
            poster: "{base}/items/{item}/poster".to_owned(),
            backdrop: "{base}/items/{item}/backdrop".to_owned(),
        },
        ports: Ports {
            upstream: UPSTREAM_PORT,
            encrypted: None,
            internal_tls: None,
        },
    }
}

crate::contract! {
    /// `stream.guard`: the guard in front of the media server.
    pub mod guard = "stream.guard" @ 1 {
        impl Guarding {
            /// Enforce an envelope in place of the one it enforces.
            fn guard(refer guarded: &Guarded as Guarded) -> () where asks = Asked {
                guarded: Guarded {
                    declared: declared(),
                    upstream: UPSTREAM.to_owned(),
                    adapter: None,
                    proxy: PROXY.to_owned(),
                    pass: PASS.to_owned(),
                },
            };
            /// The envelope it enforces.
            fn guarding() -> Option<Guarded> where asks = Asked {};
            /// The certificate it presents.
            fn certificate() -> Presented where asks = Asked {};
        }
    }
}
