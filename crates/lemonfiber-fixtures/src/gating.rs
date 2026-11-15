//! lemonfiber's own guards, held in memory as their plugins hold them.
//!
//! The request gate admits what it was last told to and answers each credential by its
//! SHA-256, and the decline service holds the key and the invitations it was last handed
//! and answers the refusals and lapses a test gives it.

use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};

use async_trait::async_trait;
use lemonfiber_ports::digest::sha256;
use lemonfiber_ports::service::{
    Admitted, Declined, Declining, Failure, GateEntry, GateRecord, Gated, Gating, Invitations,
    KeyHeld, Lapsed,
};

/// What the request gate's refusals name it as.
const GATE: &str = "the request gate";

/// What the decline service's refusals name it as.
const DECLINE: &str = "the decline service";

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
        refusal(GATE, &self.refused, "admit_tokens")?;
        let mut routes = self.routes();
        let Some(admitting) = routes.iter_mut().find(|one| one.path == route) else {
            return Err(Failure::Refused {
                service: GATE.to_owned(),
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
        refusal(GATE, &self.refused, "admit")?;
        write(&self.routes, routes.to_vec());
        Ok(())
    }

    async fn admitted(&self) -> Result<Vec<Admitted>, Failure> {
        refusal(GATE, &self.refused, "admitted")?;
        Ok(self
            .fixed
            .clone()
            .unwrap_or_else(|| self.routes().iter().map(Admitted::of).collect()))
    }

    async fn admit_tokens(&self, route: &str, tokens: &[String]) -> Result<(), Failure> {
        self.tokens_admitted(route, tokens)
    }

    async fn record(&self, after: u64) -> Result<GateRecord, Failure> {
        refusal(GATE, &self.refused, "record")?;
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
        refusal(DECLINE, &self.refused, "hold")?;
        write(&self.key, Some((key.to_owned(), pins.to_vec())));
        Ok(())
    }

    async fn held(&self) -> Result<Option<KeyHeld>, Failure> {
        refusal(DECLINE, &self.refused, "held")?;
        Ok(read(&self.key).map(|(key, pins)| KeyHeld {
            key: sha256(self.fixed.as_deref().unwrap_or(&key)),
            pins,
        }))
    }

    async fn apply_invitations(&self, invitations: &Invitations) -> Result<(), Failure> {
        refusal(DECLINE, &self.refused, "apply_invitations")?;
        write(&self.invitations, Some(invitations.clone()));
        Ok(())
    }

    async fn refusals(&self) -> Result<Vec<Declined>, Failure> {
        refusal(DECLINE, &self.refused, "refusals")?;
        Ok(self.refusals.clone())
    }

    async fn lapses(&self) -> Result<Vec<Lapsed>, Failure> {
        refusal(DECLINE, &self.refused, "lapses")?;
        Ok(self.lapses.clone())
    }

    async fn page(&self) -> Result<String, Failure> {
        refusal(DECLINE, &self.refused, "page")?;
        Ok(self.page.clone())
    }
}
