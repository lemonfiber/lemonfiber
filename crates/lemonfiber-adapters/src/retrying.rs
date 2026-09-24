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

use async_trait::async_trait;

use lemonfiber_error::retry;
use lemonfiber_ports::http::{Http, Method, Request, Response, Unreachable};

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
}

/// Ask again while it is safe to and there are attempts left.
async fn sent<H: Http + ?Sized>(inner: &H, request: &Request) -> Result<Response, Unreachable> {
    let mut attempt = 1;
    loop {
        let failure = match inner.send(request).await {
            Ok(response) => return Ok(response),
            Err(failure) => failure,
        };
        let Some(wait) = is_idempotent(request.method)
            .then(|| retry::again(attempt))
            .flatten()
        else {
            // Either not safe to repeat, or the attempts are spent. The count
            // travels with the failure so what reports it can tell a service
            // that was busy from one that is down.
            return Err(Unreachable {
                attempts: attempt,
                ..failure
            });
        };
        tokio::time::sleep(wait).await;
        attempt = attempt.saturating_add(1);
    }
}

#[cfg(test)]
mod tests;
