//! lemonfiber's own guards as their plugins answer over their contracts, held in memory.
//!
//! The request gate admits what it was last told to and answers each credential by its
//! SHA-256, and the decline service holds the key and the invitations it was last handed
//! and answers the refusals and lapses a test gives it. [`Serving`] answers both contracts
//! from them and hands every other call to the transport beside it, so a test scripts the
//! media server and the request service as it always has and reads back what each guard
//! was told.

use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use super::adapted::{Adapted, PORT};
use lemonfiber_contract::capabilities::{invite::decline, request::gate};
use lemonfiber_contract::wire;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::ports::http::{Http, Request, Response, Unreachable};
pub use lemonfiber_fixtures::gating::{Decline, Gate};
use lemonfiber_fixtures::http::Fake;

/// The header a call to a guard carries its key in.
const AUTHORIZATION: &str = "Authorization";

/// The status a guard answers a call carrying no key with.
const UNAUTHORISED: u16 = 401;

/// The decline service's plugin, speaking `invite.decline` itself.
pub const DECLINE: Adapted = Adapted {
    plugin: "decline",
    adapter: "decline",
    port: PORT,
};

/// The request gate's plugin, speaking `request.gate` itself.
pub const GATE: Adapted = Adapted {
    plugin: "request-gate",
    adapter: "request-gate",
    port: PORT,
};

/// Both guards.
pub const BOTH: [Adapted; 2] = [DECLINE, GATE];

/// The page a decline service declines an invitation on, its token in braces.
pub const PAGE: &str = "/decline/{token}";

/// Both guards as the bundle pins them, recorded as installed: what a machine holds once
/// they moved onto their plugins.
#[must_use]
pub fn both_installed() -> Vec<lemonfiber_core::plugin::Installed> {
    BOTH.into_iter().filter_map(Adapted::installed).collect()
}

/// `ctx` with both guards moved onto their plugins, each keyed and recorded as installed.
#[must_use]
pub fn moved(ctx: Ctx) -> Ctx {
    for guard in BOTH {
        guard.moved_in(&ctx);
    }
    ctx
}

/// Which guards answer `request.gate` and `invite.decline`, and what answers every other
/// call, for the transport [`Serving::transport`] builds over them.
#[derive(Clone)]
pub struct Serving {
    /// The request gate, where the stack has one.
    gate: Option<Arc<Gate>>,
    /// The decline service, where the stack has one.
    decline: Option<Arc<Decline>>,
    /// What answers every other call.
    beside: Arc<dyn Http>,
    /// Every call the guards were asked, in order.
    asked: Arc<Mutex<Vec<Request>>>,
}

impl Serving {
    /// The guards nobody runs yet, every call answered by `beside`.
    #[must_use]
    pub fn beside(beside: Arc<dyn Http>) -> Self {
        Self {
            gate: None,
            decline: None,
            beside,
            asked: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// The same guards, `request.gate` answered from `gate`.
    #[must_use]
    pub fn gate(mut self, gate: &Arc<Gate>) -> Self {
        self.gate = Some(Arc::clone(gate));
        self
    }

    /// The same guards, `invite.decline` answered from `decline`.
    #[must_use]
    pub fn decline(mut self, decline: &Arc<Decline>) -> Self {
        self.decline = Some(Arc::clone(decline));
        self
    }

    /// Every call either guard was asked, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<Request> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// A transport answering each guard's contract from it, and every other call with
    /// what is beside them.
    #[must_use]
    pub fn transport(&self) -> Arc<Fake> {
        let serving = self.clone();
        Fake::handing(move |request| {
            let serving = serving.clone();
            Box::pin(async move { serving.served(&request).await })
        })
    }

    /// What `request` is answered: by a guard where it asks one's contract, and by the
    /// transport beside them otherwise.
    async fn served(&self, request: &Request) -> Result<Response, Unreachable> {
        let body = request.body.as_deref().unwrap_or_default().as_bytes();
        let gating = Self::operation(request, gate::CAPABILITY, gate::MAJOR);
        let declining = Self::operation(request, decline::CAPABILITY, decline::MAJOR);
        match (
            self.gate.as_ref().zip(gating),
            self.decline.as_ref().zip(declining),
        ) {
            (Some((gate, operation)), _) => {
                self.guarded(request, gate::dispatch(&**gate, operation, body))
                    .await
            }
            (None, Some((decline, operation))) => {
                self.guarded(request, decline::dispatch(&**decline, operation, body))
                    .await
            }
            (None, None) => self.beside.send(request).await,
        }
    }

    /// `request` asked of a guard, recorded and answered as `dispatched` serves it, or
    /// refused unheard where it carries no key.
    async fn guarded(
        &self,
        request: &Request,
        dispatched: impl Future<Output = Result<wire::Answer, wire::Refusal>>,
    ) -> Result<Response, Unreachable> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        let keyed = request
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(AUTHORIZATION));
        if !keyed {
            return Ok(Response {
                status: UNAUTHORISED,
                headers: Vec::new(),
                body: String::new(),
            });
        }
        Ok(responded(dispatched.await))
    }

    /// The operation of `capability` `request` asks, where it asks one.
    fn operation<'r>(request: &'r Request, capability: &str, major: u32) -> Option<&'r str> {
        let prefix = lemonfiber_contract::path(capability, major, "");
        let at = request.url.find(&prefix)?;
        request.url.get(at + prefix.len()..)
    }
}

/// What a guard serving a call answers on the wire.
fn responded(served: Result<wire::Answer, wire::Refusal>) -> Response {
    match served {
        Ok(wire::Answer::Json(body)) => Response {
            status: 200,
            headers: Vec::new(),
            body: String::from_utf8(body).unwrap_or_default(),
        },
        Ok(wire::Answer::Reset) => Response {
            status: wire::RESET,
            headers: Vec::new(),
            body: String::new(),
        },
        Err(refusal) => Response {
            status: refusal.kind.status(),
            headers: vec![("Content-Type".to_owned(), wire::PROBLEM.to_owned())],
            body: serde_json::to_string(&refusal).unwrap_or_default(),
        },
    }
}

/// `ctx` reaching `gate` as the request gate's plugin: [`GATE`] moved in, the engine
/// saying it runs beside whatever else runs, and every `request.gate` call answered from
/// it.
#[must_use]
pub fn with_gate(ctx: Ctx, gate: &Arc<Gate>) -> Ctx {
    let ctx = moved_beside(ctx, GATE);
    let http = Arc::clone(&ctx.seams.http);
    ctx.with_http(Serving::beside(http).gate(gate).transport())
}

/// `ctx` reaching `decline` as the decline service's plugin: [`DECLINE`] moved in, the
/// engine saying it runs beside whatever else runs, and every `invite.decline` call
/// answered from it.
#[must_use]
pub fn with_decline(ctx: Ctx, decline: &Arc<Decline>) -> Ctx {
    let ctx = moved_beside(ctx, DECLINE);
    let http = Arc::clone(&ctx.seams.http);
    ctx.with_http(Serving::beside(http).decline(decline).transport())
}

/// `ctx` with `guard` moved in, and running beside whatever else its engine says runs.
fn moved_beside(mut ctx: Ctx, guard: Adapted) -> Ctx {
    guard.moved_in(&ctx);
    ctx.seams.engine = Arc::new(guard.running().over(Arc::clone(&ctx.seams.engine)));
    ctx
}

#[cfg(test)]
#[path = "guards/tests.rs"]
mod tests;
