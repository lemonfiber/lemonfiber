//! Speaking HTTP to an upstream, for real: a [`Request`] turned into a `reqwest` call and
//! what comes back turned into a [`Response`], bounded in time and in size.

use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::http::{Fetched, Http, Method, Request, Response, Unreachable};

/// How long reaching the upstream may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// How long one call may take, its answer read whole.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The most of an answer read.
const BODY_LIMIT: usize = 16 * 1024 * 1024;

/// An HTTP client that follows no redirect and reads no more than [`BODY_LIMIT`].
#[derive(Debug, Clone)]
pub struct Web {
    /// The client every call is sent through.
    client: reqwest::Client,
}

impl Web {
    /// A client ready to send requests, carrying no cookie from one to the next.
    ///
    /// # Errors
    ///
    /// The error building the client.
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::built(false)
    }

    /// A client that carries the session cookie an upstream sets on a sign-in to the
    /// calls after it.
    ///
    /// # Errors
    ///
    /// The error building the client.
    pub fn keeping_sessions() -> Result<Self, reqwest::Error> {
        Self::built(true)
    }

    /// A client, carrying cookies from one call to the next where `sessions` says so.
    fn built(sessions: bool) -> Result<Self, reqwest::Error> {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .cookie_store(sessions)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map(|client| Self { client })
    }
}

#[async_trait]
impl Http for Web {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        read_as_text(&self.client, request, BODY_LIMIT).await
    }

    async fn fetch(&self, request: &Request, most: usize) -> Result<Fetched, Unreachable> {
        sent(&self.client, request, most.min(BODY_LIMIT)).await
    }
}

/// What `request` was answered, read as text, refused where it is larger than `most`.
async fn read_as_text(
    client: &reqwest::Client,
    request: &Request,
    most: usize,
) -> Result<Response, Unreachable> {
    let fetched = sent(client, request, most).await?;
    let Some(bytes) = fetched.bytes else {
        return Err(Unreachable::once(
            &request.url,
            "the answer was larger than this adapter reads",
        ));
    };
    Ok(Response {
        status: fetched.status,
        headers: fetched.headers,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

/// What `request` was answered: its status, headers and up to `most` bytes of its body.
async fn sent(
    client: &reqwest::Client,
    request: &Request,
    most: usize,
) -> Result<Fetched, Unreachable> {
    let unreached = |why: reqwest::Error| Unreachable::once(&request.url, &why.to_string());
    let mut built = client.request(method(request.method), &request.url);
    for (name, value) in &request.headers {
        built = built.header(name, value);
    }
    if let Some(body) = &request.body {
        built = built.body(body.clone());
    }
    let mut answer = built.send().await.map_err(unreached)?;
    let status = answer.status().as_u16();
    let headers = answer
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.to_owned()))
        })
        .collect();
    let mut bytes = Vec::new();
    let mut within = true;
    while let Some(chunk) = answer.chunk().await.map_err(unreached)? {
        if bytes.len() + chunk.len() > most {
            within = false;
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Fetched {
        status,
        headers,
        bytes: within.then_some(bytes),
    })
}

/// The verb `reqwest` sends for `method`.
fn method(method: Method) -> reqwest::Method {
    match method {
        Method::Get => reqwest::Method::GET,
        Method::Post => reqwest::Method::POST,
        Method::Put => reqwest::Method::PUT,
        Method::Patch => reqwest::Method::PATCH,
        Method::Delete => reqwest::Method::DELETE,
    }
}

#[cfg(test)]
mod tests;
