//! An HTTP transport that writes down what left this machine.
//!
//! The enumeration beside it says what *would* be sent — which requests exist,
//! where each goes, what travels and how to stop it. That is a description of the
//! program, and an operator checking whether they were told the truth needs the
//! other thing: a record of what actually went, made as it went.
//!
//! Wrapped around whatever really speaks HTTP rather than written into each
//! caller, for the reason the retry policy beside it is: a record kept at fifteen
//! call sites is fifteen records, and the request that goes unrecorded will be the
//! one at the site somebody forgot.
//!
//! **What is written down is deliberately less than what was sent.** A request
//! carries the credential a service authenticates with and, where it writes, a
//! body. A log holding either would be the thing this feature exists to prevent —
//! an operator who turned every outbound request off would still have a file full
//! of their own keys. So the line is when, what kind of request, where it went with
//! the query and any userinfo withheld, and what came back. Never a header, never
//! a body.
//!
//! Both, and in that order, because a URL carries a credential in two places and
//! only one of them is a guess. The query goes wholesale — which parameter holds a
//! key belongs to whoever wrote the service — and the password in front of the host
//! goes because the URI syntax itself says everything after that colon is one. The
//! address then goes through the same scrubber a support bundle's text does, so a
//! credential that reached a URL in spite of all this is withheld here too rather
//! than only where somebody remembered.

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::error::withheld::{withheld, without_credentials};
use crate::ports::http::{Http, Request, Response, Unreachable};
use crate::ports::time::Clock;

mod inside;
mod ledger;

pub use inside::Inside;
pub use ledger::Ledger;

/// A transport that writes down what it sent.
pub struct Recording<H> {
    /// What actually speaks HTTP.
    inner: H,
    /// Where the record is kept, or nothing where this machine will not say where
    /// its own files go — in which case nothing is written and nothing pretends to
    /// have been.
    ledger: Option<Arc<Ledger>>,
    /// What the time is, asked through the port so a test can say.
    clock: Arc<dyn Clock>,
}

impl<H> Recording<H> {
    /// Wrap a transport so what it sends is written down at `at`.
    #[must_use]
    pub fn around(inner: H, at: Option<PathBuf>, clock: Arc<dyn Clock>) -> Self {
        Self::sharing(inner, at.map(|at| Arc::new(Ledger::at(at))), clock)
    }

    /// Wrap a transport so what it sends is written down in a record something else
    /// writes to as well.
    pub const fn sharing(inner: H, ledger: Option<Arc<Ledger>>, clock: Arc<dyn Clock>) -> Self {
        Self {
            inner,
            ledger,
            clock,
        }
    }
}

/// Whether a request to this address leaves the machine at all.
///
/// The question the record answers is what left, and a service of the stack's own
/// reached on this machine's loopback address went nowhere. Written down, those
/// reads would be most of the record — the dashboard alone makes dozens a second —
/// and would push a request that really did leave out of it within a minute.
pub(crate) fn leaves_this_machine(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host_and_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match host_and_port.strip_prefix('[') {
        Some(bracketed) => bracketed.split(']').next().unwrap_or_default(),
        None => host_and_port.split(':').next().unwrap_or_default(),
    };
    let named_here =
        host.eq_ignore_ascii_case("localhost") || host.to_ascii_lowercase().ends_with(".localhost");
    let addressed_here = host
        .parse::<IpAddr>()
        .is_ok_and(|address| address.is_loopback() || address.is_unspecified());
    !(named_here || addressed_here)
}

/// What the time is, as the record writes it: whole seconds since the epoch.
pub(crate) fn stamped(clock: &dyn Clock) -> u64 {
    clock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

/// One line of the record.
///
/// Built here rather than at the write, so what a line contains is one function a
/// test can put a request to — and so the rule that a header never reaches it is a
/// property of a value rather than of a habit.
///
/// The address goes through [`without_credentials`] and then [`withheld`], and that
/// order is the whole of it rather than a tidiness. Both places a URL can carry a
/// credential — the query, and the password in front of the host — are withheld by
/// the first; the second is the scrubber a support bundle's text takes, kept after
/// it so a credential shaped like a setting inside the path is caught too. The query
/// is not taken off before either of them: the general scrubber reaches the address
/// rule only through a token that still carries a `?…=…`, so a login written in
/// front of the host would fall through both and be written down verbatim.
///
/// Withheld rather than deleted, so the line still says a query was sent — a record
/// that quietly drops the fact reads as a smaller request than the one that left.
fn line(at: u64, request: &Request, answered: Option<u16>) -> String {
    let outcome = answered.map_or_else(
        || "nothing answered".to_owned(),
        |status| status.to_string(),
    );
    format!(
        "{at} {:?} {} {outcome}",
        request.method,
        withheld(&without_credentials(&request.url))
    )
}

#[async_trait]
impl<H: Http + Send + Sync> Http for Recording<H> {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        send(self, request).await
    }
}

async fn send<H: Http + Send + Sync>(
    recording: &Recording<H>,
    request: &Request,
) -> Result<Response, Unreachable> {
    let answer = recording.inner.send(request).await;
    let Some(ledger) = recording
        .ledger
        .as_ref()
        .filter(|_| leaves_this_machine(&request.url))
    else {
        return answer;
    };
    let status = answer.as_ref().ok().map(|answered| answered.status);
    let when = stamped(recording.clock.as_ref());
    ledger.note(line(when, request, status)).await;
    answer
}

/// The record is asserted about without being quoted.
///
/// Most of what is checked here is that a credential did **not** survive into a
/// line, and a failure message is copied into a CI log — read by more people and
/// kept far longer than the machine that wrote it. An assertion that a key is
/// absent must not be the thing that carries it onward when it is wrong, which is
/// the one moment the line actually holds one.
///
/// So no assertion below interpolates the line it is unhappy with. Each names the
/// half of the claim that broke instead, which is what a reader of the failure
/// needs and is the half `{said}` never told them.
#[cfg(test)]
mod tests;
