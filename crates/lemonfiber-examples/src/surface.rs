//! The web surface over one world, asked as a client asks it.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use axum::Router;
use lemonfiber_api::events::live::{Listening, Live};
use lemonfiber_api::events::wire::BEAT_SAID;
use lemonfiber_api::events::Streaming;
use lemonfiber_api::guard::{Arrived, Binding, Token, TOKEN_HEADER};
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::{routes, Serving};
use lemonfiber_core::app::admitting::AtHand;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::model::kind;
use lemonfiber_fixtures::ports::{Chance, Stopped};
use serde_json::Value;
use tower::ServiceExt as _;

use crate::world::{World, AT};

/// The port the surface is bound to, as every request names it.
const PORT: u16 = 8471;

/// The bytes the run's own token is drawn from: other than those every session this
/// world opens is drawn from, so a session is never the run's token.
const RUN: &[u8] = b"the-run";

/// Where a session is opened by a password.
const SESSION: &str = "/api/session";

/// How often a job is asked about while it is still going.
const STEP: Duration = Duration::from_millis(250);

/// How many times a job is asked about before it is released.
const PATIENCE: usize = 240;

/// What one request was answered with.
pub(crate) struct Answer {
    /// The status it was answered with.
    pub(crate) status: StatusCode,
    /// Every envelope the body held, in order.
    pub(crate) envelopes: Vec<Value>,
}

impl Answer {
    /// What a request that could not be made or read back stands as.
    const fn unanswered() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            envelopes: Vec::new(),
        }
    }
}

/// The surface over one world, with a listener on its stream.
pub(crate) struct Surface {
    /// Every route, behind the guard.
    router: Router,
    /// The token every request carries.
    token: String,
    /// A listener that opened before anything was asked.
    listening: Listening,
    /// The live stream the surface says things on.
    live: Arc<Live>,
    /// The world it answers in.
    world: World,
}

impl Surface {
    /// The surface over `world`.
    pub(crate) async fn over(world: World) -> Option<Self> {
        let ctx = world.ctx.clone();
        Self::serving(world, ctx).await
    }

    /// The surface over `world`, asking `household` who somebody is.
    pub(crate) async fn over_household(world: World, household: Arc<dyn AtHand>) -> Option<Self> {
        let ctx = world.ctx.clone().with_household(household);
        Self::serving(world, ctx).await
    }

    /// The surface over `world`, dispatching through `ctx`.
    async fn serving(world: World, ctx: Ctx) -> Option<Self> {
        let token = Arc::new(Token::mint(&Chance::repeating(RUN.to_vec()))?);
        let clock = Stopped::at(AT);
        let live = Arc::new(Live::opening(clock.as_ref()));
        let listening = live.listening(None).await;
        let ctx = Arc::new(ctx);
        let serving = Serving::for_run(
            Arc::clone(&ctx),
            Arc::clone(&token),
            Binding::here(PORT),
            Jobs::default(),
            Arc::clone(&live),
        );
        let streaming = Arc::new(Streaming {
            token: Arc::clone(&token),
            bound: Binding::here(PORT),
            live: Arc::clone(&live),
            clock,
            reading: ctx,
        });
        Some(Self {
            router: routes(serving, streaming),
            token: token.as_str().to_owned(),
            listening,
            live,
            world,
        })
    }

    /// The world it answers in.
    pub(crate) const fn world(&self) -> &World {
        &self.world
    }

    /// The live stream it says things on.
    pub(crate) fn live(&self) -> &Live {
        &self.live
    }

    /// What `method` at `path`, carrying `body`, was answered with.
    pub(crate) async fn asked(&self, method: &str, path: &str, body: Option<&Value>) -> Answer {
        self.asked_as(&self.token, method, path, body).await
    }

    /// The same, carrying `token` in place of the run's own.
    pub(crate) async fn asked_as(
        &self,
        token: &str,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Answer {
        self.sent(token, method, path, body)
            .await
            .unwrap_or(Answer::unanswered())
    }

    /// The token of the session `name` opens with `password`, and what opening it said,
    /// or nothing where none was opened.
    pub(crate) async fn signed_in(&self, name: &str, password: &str) -> (Option<String>, Answer) {
        let given = serde_json::json!({ "name": name, "password": password });
        let answer = self.asked("POST", SESSION, Some(&given)).await;
        let token = answer
            .envelopes
            .first()
            .and_then(|envelope| envelope.pointer("/data/token"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        (token, answer)
    }

    /// The same, or nothing where the request could not be made or read back.
    async fn sent(
        &self,
        token: &str,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Option<Answer> {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(TOKEN_HEADER, token)
            .header(header::CONTENT_TYPE, "application/json")
            .extension(Arrived {
                from: IpAddr::V4(Ipv4Addr::LOCALHOST),
                encrypted: false,
            })
            .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
            .ok()?;
        let response = self.router.clone().oneshot(request).await.ok()?;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.ok()?;
        Some(Answer {
            status,
            envelopes: self.read(&String::from_utf8_lossy(&bytes)),
        })
    }

    /// What the work an envelope names came to, or nothing where it names no work.
    ///
    /// Work still going after [`PATIENCE`] looks came to what the last look answered.
    pub(crate) async fn redeemed(&self, envelope: &Value) -> Vec<Value> {
        let Some(job) = named_work(envelope) else {
            return Vec::new();
        };
        let at = format!("/api/jobs/{job}");
        let mut looks = 0;
        let answer = loop {
            tokio::time::sleep(STEP).await;
            looks += 1;
            let answer = self.asked("GET", &at, None).await;
            if answer.status != StatusCode::ACCEPTED || looks == PATIENCE {
                break answer;
            }
        };
        answer.envelopes
    }

    /// Every envelope said on the stream since this was last asked.
    pub(crate) async fn heard(&mut self) -> Vec<Value> {
        let mut heard = Vec::new();
        while let Some(said) = self.listening.next().await {
            if said == BEAT_SAID {
                break;
            }
            let data: Vec<&str> = said
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .collect();
            heard.extend(self.read(&data.join("\n")));
        }
        heard
    }

    /// Every envelope in `text`, one to a line, with the world's directory placed.
    fn read(&self, text: &str) -> Vec<Value> {
        self.world
            .placed(text)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
}

/// The name of the work `envelope` stands for, where it is a `job`.
fn named_work(envelope: &Value) -> Option<String> {
    (envelope.get("kind")?.as_str()? == kind::JOB.as_str())
        .then(|| envelope.pointer("/data/job")?.as_str().map(str::to_owned))
        .flatten()
}
