//! A transport that addresses every request as the stack addresses a service: by the
//! port it listens on, whichever port the loopback interface reaches it at.

use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::http::{Fetched, Http, Request, Response, Unreachable};

/// The header a request names the service it is addressed to in.
const HOST: &str = "Host";

/// `inner`, every request through it addressed to `host`.
pub(super) struct Addressed {
    inner: Arc<dyn Http>,
    host: String,
}

impl Addressed {
    /// `inner`, addressing the service listening on `port` as `host` does.
    pub(super) fn to(inner: Arc<dyn Http>, host: &str, port: Option<u16>) -> Arc<dyn Http> {
        let host = port.map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"));
        Arc::new(Self { inner, host })
    }

    /// `request`, addressed.
    fn addressed(&self, request: &Request) -> Request {
        let mut addressed = request.clone();
        addressed
            .headers
            .retain(|(name, _)| !name.eq_ignore_ascii_case(HOST));
        addressed.headers.push((HOST.to_owned(), self.host.clone()));
        addressed
    }
}

#[async_trait]
impl Http for Addressed {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        self.inner.send(&self.addressed(request)).await
    }

    async fn fetch(&self, request: &Request, most: usize) -> Result<Fetched, Unreachable> {
        self.inner.fetch(&self.addressed(request), most).await
    }
}

#[cfg(test)]
mod tests;
