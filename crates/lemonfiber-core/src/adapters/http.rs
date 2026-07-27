//! Speaking HTTP to services, for real.
//!
//! The whole of this adapter is translation: turn a [`Request`] into a `reqwest`
//! call, turn what comes back into a [`Response`], and turn a transport failure
//! into [`Unreachable`]. No decisions — those belong above the port, where a fake
//! stands in for this. `reqwest` is confined here and nowhere else.

use async_trait::async_trait;

use crate::ports::http::{Http, Method, Request, Response, Unreachable};

/// An HTTP client backed by `reqwest`, with rustls so the static Linux build
/// needs no system TLS library.
#[derive(Debug, Clone)]
pub struct Web {
    client: reqwest::Client,
}

impl Web {
    /// A client ready to send requests.
    ///
    /// The builder fails only where the TLS backend cannot initialise — a
    /// process-wide impossibility with rustls, not a per-request condition — so
    /// construction stays infallible for callers rather than threading a `Result`
    /// through every use.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder().build().unwrap_or_default(),
        }
    }
}

impl Default for Web {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Http for Web {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        let mut builder = match request.method {
            Method::Get => self.client.get(&request.url),
            Method::Post => self.client.post(&request.url),
        };
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }

        let unreachable = |error: &reqwest::Error| Unreachable {
            url: request.url.clone(),
            reason: error.to_string(),
        };

        // The status is read before the body, because a body that fails to arrive
        // still leaves the status known — but the port reports one Response or
        // none, so a truncated body is a failure to reach rather than a partial
        // answer.
        let response = builder.send().await.map_err(|error| unreachable(&error))?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|error| unreachable(&error))?;
        Ok(Response { status, body })
    }
}
