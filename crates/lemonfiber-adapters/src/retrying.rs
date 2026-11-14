//! An HTTP transport that tries again before giving up.
//!
//! Wrapped around whatever really speaks HTTP, rather than written into each
//! caller: a retry policy applied at fifteen call sites is fifteen policies, and
//! by the third one somebody will have picked a different number of attempts for
//! no reason anybody can reconstruct later.
//!
//! Only reads and replaces are retried. A `POST` that got no answer may still
//! have been received and acted on — the response is what went missing, not
//! necessarily the request — so retrying one risks creating the same root folder
//! or download client twice. `GET` and `PUT` are idempotent by definition, which
//! is exactly the property that makes trying again safe.
//!
//! A status code is never retried, because a status code is an answer. A service
//! that says `401` has spoken, and asking it again three times is neither more
//! polite nor more informative.
//!
//! Nor is a service that took the connection and then said nothing. What a retry
//! cures is a connection that could not be made a moment ago — a container
//! restarting, a name resolving late. One that was made and went silent has
//! already been given the whole wait, and asking it again would spend that wait
//! twice more before saying the same thing.
//!
//! A request's deadline holds across every attempt: each one asked again carries what
//! is left of it, and none is made once nothing is.

use std::time::Duration;

use async_trait::async_trait;

use lemonfiber_error::retry;
use lemonfiber_ports::http::{Fetched, Http, Method, Request, Response, Unreachable};

/// A transport that retries what is safe to retry.
pub struct Retrying<H> {
    inner: H,
}

impl<H> Retrying<H> {
    /// Wrap a transport so its idempotent requests survive a blip.
    pub const fn around(inner: H) -> Self {
        Self { inner }
    }
}

/// Whether asking again could do anything the first ask did not already do.
///
/// A read and a replace land on the same state however many times they arrive; a
/// create does not.
const fn is_idempotent(method: Method) -> bool {
    matches!(method, Method::Get | Method::Put)
}

#[async_trait]
impl<H: Http> Http for Retrying<H> {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        sent(&self.inner, request).await
    }

    async fn send_octets(&self, request: &Request, octets: &[u8]) -> Result<Response, Unreachable> {
        self.inner.send_octets(request, octets).await
    }

    async fn fetch(&self, request: &Request, most: usize) -> Result<Fetched, Unreachable> {
        self.inner.fetch(request, most).await
    }
}

/// Ask again while it is safe to, there are attempts left, and the deadline has time left.
async fn sent<H: Http + ?Sized>(inner: &H, request: &Request) -> Result<Response, Unreachable> {
    let started = tokio::time::Instant::now();
    let mut asked = request.clone();
    let mut attempt = 1;
    loop {
        let failure = match inner.send(&asked).await {
            Ok(response) => return Ok(response),
            Err(failure) => failure,
        };
        let Some((wait, again)) = (is_idempotent(request.method) && !failure.connected)
            .then(|| retry::again(attempt))
            .flatten()
            .and_then(|wait| {
                left(request, started.elapsed().saturating_add(wait)).map(|again| (wait, again))
            })
        else {
            // Not safe to repeat, not a blip, or the attempts are spent. The count
            // travels with the failure so what reports it can tell a service
            // that was busy from one that is down.
            return Err(Unreachable {
                attempts: attempt,
                ..failure
            });
        };
        tokio::time::sleep(wait).await;
        asked = again;
        attempt = attempt.saturating_add(1);
    }
}

/// `request` carrying what is left of its deadline once `spent` has gone, or nothing where
/// none is left.
fn left(request: &Request, spent: Duration) -> Option<Request> {
    let Some(deadline) = request.deadline else {
        return Some(request.clone());
    };
    deadline
        .checked_sub(spent)
        .filter(|left| !left.is_zero())
        .map(|left| Request {
            deadline: Some(left),
            ..request.clone()
        })
}

#[cfg(test)]
mod tests;
