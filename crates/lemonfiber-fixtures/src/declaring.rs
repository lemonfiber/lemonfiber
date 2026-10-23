//! A media server that says what a guard in front of it enforces, as a test scripts it.

use async_trait::async_trait;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Addressee, Answers, BytePath, Declared, Declaring, Failure, Held, LivePath, Locations, Naming,
    Ports, Presentation, Question, Statuses, Token,
};

/// The name a server that will not answer is reported under.
const SERVICE: &str = "media";

/// Answers its declaration, or nothing where it will not say.
pub struct Declares(Option<Declared>);

impl Declares {
    /// A server declaring `declared`.
    #[must_use]
    pub const fn declaring(declared: Declared) -> Self {
        Self(Some(declared))
    }

    /// A server that will not say.
    #[must_use]
    pub const fn silent() -> Self {
        Self(None)
    }
}

#[async_trait]
impl Declaring for Declares {
    async fn declared(&self) -> Result<Declared, Failure> {
        self.0.clone().ok_or_else(|| Failure::Unavailable {
            service: SERVICE.to_owned(),
        })
    }
}

/// A declaration a guard can enforce, serving bytes under one path, of no server in
/// particular.
#[must_use]
pub fn enforceable() -> Declared {
    Declared {
        bytes: vec![BytePath {
            pattern: "^/media/(?P<item>[^/]+)/bytes$".to_owned(),
            item: "^[0-9a-f]{32}$".to_owned(),
            exempt: Vec::new(),
            naming: vec![Naming {
                parameter: "other".to_owned(),
                held: Held::Refused,
            }],
        }],
        live: vec![LivePath {
            pattern: "^/media/live/(?P<stream>[^/]+)/".to_owned(),
            stream: "^[0-9a-f]{32}$".to_owned(),
        }],
        refused: Vec::new(),
        question: Some(Question {
            method: Method::Get,
            path: "/media/{item}".to_owned(),
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
            shape: "^[0-9a-f]{32}$".to_owned(),
            parameters: vec!["key".to_owned()],
        },
        locations: Locations {
            stream: "{base}/media/{item}/bytes".to_owned(),
            poster: "{base}/media/{item}/poster".to_owned(),
            backdrop: "{base}/media/{item}/backdrop".to_owned(),
        },
        ports: Ports {
            upstream: 8080,
            encrypted: None,
        },
    }
}
