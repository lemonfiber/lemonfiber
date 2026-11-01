//! lemonfiber's own guards as their plugins answer over their contracts, held in memory.
//!
//! The request gate admits what it was last told to and answers each credential by its
//! SHA-256, and the decline service holds the key and the invitations it was last handed
//! and answers the refusals and lapses a test gives it. [`Serving`] answers both contracts
//! from them and hands every other call to the transport beside it, so a test scripts the
//! media server and the request service as it always has and reads back what each guard
//! was told.

use std::collections::BTreeSet;
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use super::adapted::{Adapted, PORT};
use async_trait::async_trait;
use lemonfiber_contract::capabilities::{invite::decline, request::gate};
use lemonfiber_contract::wire;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::ports::digest::sha256;
use lemonfiber_core::ports::docker::{
    Container, Engine, ExecOutput, Failure as EngineFailure, LogLine, LogQuery, Stats,
};
use lemonfiber_core::ports::http::{Http, Request, Response, Unreachable};
use lemonfiber_core::ports::service::{
    Admitted, Declined, Declining, Failure, GateEntry, GateRecord, Gated, Gating, Invitations,
    KeyHeld, Lapsed,
};
use lemonfiber_fixtures::support::Reporting;
use tokio::sync::mpsc::Receiver;

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

/// The value behind `held`, or what it held when a test thread panicked holding it.
fn read<T: Clone>(held: &Mutex<T>) -> T {
    held.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// `held` set to `value`.
fn write<T>(held: &Mutex<T>, value: T) {
    *held.lock().unwrap_or_else(PoisonError::into_inner) = value;
}

/// The refusal `service` answers `operation` with, where it is one of `refused`.
fn refusal(
    service: &str,
    refused: &BTreeSet<&'static str>,
    operation: &str,
) -> Result<(), Failure> {
    if refused.contains(operation) {
        return Err(Failure::Refused {
            service: service.to_owned(),
            detail: format!("it refused {operation}"),
        });
    }
    Ok(())
}

/// The request gate, as its plugin answers `request.gate`.
#[derive(Default)]
pub struct Gate {
    /// The routes it admits, as it was last told them.
    routes: Mutex<Vec<Gated>>,
    /// What it answers it admits whatever it is told, where a test fixes that.
    fixed: Option<Vec<Admitted>>,
    /// Its record of refusals and forwarded removals, the oldest it still keeps first.
    record: Vec<GateEntry>,
    /// The operations it refuses.
    refused: BTreeSet<&'static str>,
}

impl Gate {
    /// A gate admitting `routes`.
    #[must_use]
    pub fn admitting(routes: Vec<Gated>) -> Self {
        Self {
            routes: Mutex::new(routes),
            ..Self::default()
        }
    }

    /// The same gate, answering it admits `admitted` whatever it is told.
    #[must_use]
    pub fn answering(mut self, admitted: Vec<Admitted>) -> Self {
        self.fixed = Some(admitted);
        self
    }

    /// The same gate, keeping `record`, the oldest entry first.
    #[must_use]
    pub fn recording(mut self, record: Vec<GateEntry>) -> Self {
        self.record = record;
        self
    }

    /// The same gate, refusing each of `operations`.
    #[must_use]
    pub fn refusing(mut self, operations: &[&'static str]) -> Self {
        self.refused.extend(operations);
        self
    }

    /// Every route it admits now, as it was last told them.
    #[must_use]
    pub fn routes(&self) -> Vec<Gated> {
        read(&self.routes)
    }

    /// The token hashes it admits on the route under `path`, where it has one.
    #[must_use]
    pub fn tokens(&self, path: &str) -> Option<Vec<String>> {
        self.routes()
            .into_iter()
            .find(|route| route.path == path)
            .map(|route| route.tokens)
    }

    /// Admit `tokens` under `route`, refused where it admits no such route.
    fn tokens_admitted(&self, route: &str, tokens: &[String]) -> Result<(), Failure> {
        refusal(gate::CAPABILITY, &self.refused, "admit_tokens")?;
        let mut routes = self.routes();
        let Some(admitting) = routes.iter_mut().find(|one| one.path == route) else {
            return Err(Failure::Refused {
                service: gate::CAPABILITY.to_owned(),
                detail: format!("it admits no route under {route}"),
            });
        };
        admitting.tokens = tokens.to_vec();
        write(&self.routes, routes);
        Ok(())
    }
}

#[async_trait]
impl Gating for Gate {
    async fn admit(&self, routes: &[Gated]) -> Result<(), Failure> {
        refusal(gate::CAPABILITY, &self.refused, "admit")?;
        write(&self.routes, routes.to_vec());
        Ok(())
    }

    async fn admitted(&self) -> Result<Vec<Admitted>, Failure> {
        refusal(gate::CAPABILITY, &self.refused, "admitted")?;
        Ok(self
            .fixed
            .clone()
            .unwrap_or_else(|| self.routes().iter().map(Admitted::of).collect()))
    }

    async fn admit_tokens(&self, route: &str, tokens: &[String]) -> Result<(), Failure> {
        self.tokens_admitted(route, tokens)
    }

    async fn record(&self, after: u64) -> Result<GateRecord, Failure> {
        refusal(gate::CAPABILITY, &self.refused, "record")?;
        let entries: Vec<GateEntry> = self
            .record
            .iter()
            .filter(|entry| entry.seq > after)
            .cloned()
            .collect();
        let lost = entries
            .first()
            .map_or(0, |first| first.seq.saturating_sub(after + 1));
        Ok(GateRecord { entries, lost })
    }
}

/// The decline service, as its plugin answers `invite.decline`.
#[derive(Default)]
pub struct Decline {
    /// The key it was handed and the certificates it accepts, the key itself.
    key: Mutex<Option<(String, Vec<String>)>>,
    /// The key it answers it holds whatever it is handed, where a test fixes that.
    fixed: Option<String>,
    /// The invitations it was last handed.
    invitations: Mutex<Option<Invitations>>,
    /// The invitations it declined.
    refusals: Vec<Declined>,
    /// The invitations it took back, or looked at and left, as their windows closed.
    lapses: Vec<Lapsed>,
    /// Where it serves the page an invitation is declined on.
    page: String,
    /// The operations it refuses.
    refused: BTreeSet<&'static str>,
}

impl Decline {
    /// A decline service holding `key`, accepting `pins`.
    #[must_use]
    pub fn holding(key: &str, pins: &[&str]) -> Self {
        Self {
            key: Mutex::new(Some((
                key.to_owned(),
                pins.iter().map(|pin| (*pin).to_owned()).collect(),
            ))),
            ..Self::default()
        }
    }

    /// The same service, answering it holds `key` whatever it is handed.
    #[must_use]
    pub fn answering(mut self, key: &str) -> Self {
        self.fixed = Some(key.to_owned());
        self
    }

    /// The same service, handed `invitations` already.
    #[must_use]
    pub fn handed(self, invitations: Invitations) -> Self {
        write(&self.invitations, Some(invitations));
        self
    }

    /// The same service, answering it declined `refusals`.
    #[must_use]
    pub fn declining(mut self, refusals: Vec<Declined>) -> Self {
        self.refusals = refusals;
        self
    }

    /// The same service, answering it took back or left `lapses`.
    #[must_use]
    pub fn lapsing(mut self, lapses: Vec<Lapsed>) -> Self {
        self.lapses = lapses;
        self
    }

    /// The same service, serving its page at `page`.
    #[must_use]
    pub fn paging(mut self, page: &str) -> Self {
        page.clone_into(&mut self.page);
        self
    }

    /// The same service, refusing each of `operations`.
    #[must_use]
    pub fn refusing(mut self, operations: &[&'static str]) -> Self {
        self.refused.extend(operations);
        self
    }

    /// The key it holds, the key itself, where it holds one.
    #[must_use]
    pub fn key(&self) -> Option<String> {
        read(&self.key).map(|(key, _)| key)
    }

    /// The certificates it accepts, each by its SHA-256.
    #[must_use]
    pub fn pins(&self) -> Vec<String> {
        read(&self.key).map(|(_, pins)| pins).unwrap_or_default()
    }

    /// The invitations it was last handed, where it was handed any.
    #[must_use]
    pub fn invitations(&self) -> Option<Invitations> {
        read(&self.invitations)
    }
}

#[async_trait]
impl Declining for Decline {
    async fn hold(&self, key: &str, pins: &[String]) -> Result<(), Failure> {
        refusal(decline::CAPABILITY, &self.refused, "hold")?;
        write(&self.key, Some((key.to_owned(), pins.to_vec())));
        Ok(())
    }

    async fn held(&self) -> Result<Option<KeyHeld>, Failure> {
        refusal(decline::CAPABILITY, &self.refused, "held")?;
        Ok(read(&self.key).map(|(key, pins)| KeyHeld {
            key: sha256(self.fixed.as_deref().unwrap_or(&key)),
            pins,
        }))
    }

    async fn apply_invitations(&self, invitations: &Invitations) -> Result<(), Failure> {
        refusal(decline::CAPABILITY, &self.refused, "apply_invitations")?;
        write(&self.invitations, Some(invitations.clone()));
        Ok(())
    }

    async fn refusals(&self) -> Result<Vec<Declined>, Failure> {
        refusal(decline::CAPABILITY, &self.refused, "refusals")?;
        Ok(self.refusals.clone())
    }

    async fn lapses(&self) -> Result<Vec<Lapsed>, Failure> {
        refusal(decline::CAPABILITY, &self.refused, "lapses")?;
        Ok(self.lapses.clone())
    }

    async fn page(&self) -> Result<String, Failure> {
        refusal(decline::CAPABILITY, &self.refused, "page")?;
        Ok(self.page.clone())
    }
}

/// A transport answering `request.gate` and `invite.decline` from the guards it holds,
/// and handing every other call to the one beside it.
pub struct Serving {
    /// The request gate, where the stack has one.
    gate: Option<Arc<Gate>>,
    /// The decline service, where the stack has one.
    decline: Option<Arc<Decline>>,
    /// What answers every other call.
    beside: Arc<dyn Http>,
    /// Every call the guards were asked, in order.
    asked: Mutex<Vec<Request>>,
}

impl Serving {
    /// The guards nobody runs yet, every call answered by `beside`.
    #[must_use]
    pub fn beside(beside: Arc<dyn Http>) -> Self {
        Self {
            gate: None,
            decline: None,
            beside,
            asked: Mutex::new(Vec::new()),
        }
    }

    /// The same transport, answering `request.gate` from `gate`.
    #[must_use]
    pub fn gate(mut self, gate: &Arc<Gate>) -> Self {
        self.gate = Some(Arc::clone(gate));
        self
    }

    /// The same transport, answering `invite.decline` from `decline`.
    #[must_use]
    pub fn decline(mut self, decline: &Arc<Decline>) -> Self {
        self.decline = Some(Arc::clone(decline));
        self
    }

    /// Every call either guard was asked, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<Request> {
        read(&self.asked)
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
        Some(&request.url[at + prefix.len()..])
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

#[async_trait]
impl Http for Serving {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        self.served(request).await
    }
}

/// An engine saying what `beside` runs alongside whatever the engine it wraps says runs,
/// that engine holding nothing where it cannot be reached; everything else asked of the
/// engine it wraps.
pub struct Alongside {
    /// The engine every other question goes to.
    engine: Arc<dyn Engine>,
    /// What runs beside it.
    beside: Reporting,
}

impl Alongside {
    /// `beside` running alongside what `engine` says runs.
    #[must_use]
    pub fn new(engine: Arc<dyn Engine>, beside: Reporting) -> Self {
        Self { engine, beside }
    }
}

#[async_trait]
impl Engine for Alongside {
    async fn list(&self, project: &str) -> Result<Vec<Container>, EngineFailure> {
        let mut running = self.engine.list(project).await.unwrap_or_default();
        running.extend(self.beside.list(project).await?);
        Ok(running)
    }

    async fn exec(&self, container: &str, argv: &[String]) -> Result<ExecOutput, EngineFailure> {
        self.engine.exec(container, argv).await
    }

    async fn stats(&self, project: &str) -> Result<Receiver<(String, Stats)>, EngineFailure> {
        self.engine.stats(project).await
    }

    async fn logs(
        &self,
        project: &str,
        services: &[String],
        query: LogQuery,
    ) -> Result<Receiver<LogLine>, EngineFailure> {
        self.engine.logs(project, services, query).await
    }
}

/// `ctx` reaching `gate` as the request gate's plugin: [`GATE`] moved in, the engine
/// saying it runs beside whatever else runs, and every `request.gate` call answered from
/// it.
#[must_use]
pub fn with_gate(ctx: Ctx, gate: &Arc<Gate>) -> Ctx {
    let ctx = moved_beside(ctx, GATE);
    let http = Arc::clone(&ctx.seams.http);
    ctx.with_http(Arc::new(Serving::beside(http).gate(gate)))
}

/// `ctx` reaching `decline` as the decline service's plugin: [`DECLINE`] moved in, the
/// engine saying it runs beside whatever else runs, and every `invite.decline` call
/// answered from it.
#[must_use]
pub fn with_decline(ctx: Ctx, decline: &Arc<Decline>) -> Ctx {
    let ctx = moved_beside(ctx, DECLINE);
    let http = Arc::clone(&ctx.seams.http);
    ctx.with_http(Arc::new(Serving::beside(http).decline(decline)))
}

/// `ctx` with `guard` moved in, and running beside whatever else its engine says runs.
fn moved_beside(mut ctx: Ctx, guard: Adapted) -> Ctx {
    guard.moved_in(&ctx);
    ctx.seams.engine = Arc::new(Alongside::new(
        Arc::clone(&ctx.seams.engine),
        guard.running(),
    ));
    ctx
}

#[cfg(test)]
#[path = "guards/tests.rs"]
mod tests;
