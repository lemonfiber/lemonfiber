//! Speaking HTTP to services, for real.
//!
//! The whole of this adapter is translation: turn a [`Request`] into a `reqwest`
//! call, turn what comes back into a [`Response`], and turn a transport failure
//! into [`Unreachable`]. No decisions — those belong above the port, where a fake
//! stands in for this. `reqwest` is confined here and nowhere else.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use reqwest::header::HeaderValue;
use reqwest::Url;

use crate::config::store::is_secret;
use crate::ports::http::{Http, Method, Request, Response, Unreachable};

/// How long to wait for a service to accept a connection before treating it as
/// not answering. One that is up answers a local connection at once; the wait is
/// only ever spent on one that is not there.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait for a whole request to complete. Generous enough for a
/// service still warming up on first contact, bounded so a socket that accepts a
/// connection but never sends a reply cannot hang a seed run without end.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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
    ///
    /// A cookie store is kept because one service needs it: qBittorrent's web UI
    /// authenticates a session by a cookie set at login and expected on the calls
    /// that follow. It is [`PerOrigin`] rather than the default one, which would
    /// hand that session to every other service on the machine — see there for why.
    #[must_use]
    pub fn new() -> Self {
        // `build` only fails if the TLS backend cannot initialise, which the bundled
        // rustls provider does not do at runtime — so this is infallible in practice.
        // The `unwrap_or_default` names that rather than `expect` (which the lints
        // forbid); were it ever to fire, the default client would drop the cookie
        // store and timeouts these lines set, so the fallback is a degraded client,
        // not an equivalent one — acceptable only because it is unreachable.
        Self {
            client: reqwest::Client::builder()
                .cookie_provider(Arc::new(PerOrigin::default()))
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_default(),
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
            Method::Put => self.client.put(&request.url),
        };
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }

        // A transport error's own words can quote the URL it failed on, and a URL
        // can carry a secret in its query — an indexer authenticates by an `apikey`
        // parameter, not a header. Mask those values wherever they would otherwise
        // reach an operator: in the URL kept on the failure and in the reason read
        // from the error.
        let secrets = query_secrets(&request.url);
        let unreachable = |error: &reqwest::Error| Unreachable {
            url: mask(&request.url, &secrets),
            reason: mask(&error.to_string(), &secrets),
            attempts: 1,
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

/// The values of any query parameters whose names mark them secret — an indexer's
/// `apikey` above all. Read from the request URL so a transport error quoting that
/// URL can have them masked before it is surfaced. The same name-based rule the
/// configuration store redacts by decides what counts (`apikey` holds `KEY`), so
/// there is one answer to "is this a secret" rather than two.
fn query_secrets(url: &str) -> Vec<&str> {
    let Some((_, query)) = url.split_once('?') else {
        return Vec::new();
    };
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .filter(|(name, value)| is_secret(name) && !value.is_empty())
        .map(|(_, value)| value)
        .collect()
}

/// `text` with every one of `secrets` blotted out — what makes a URL or an error
/// message safe to show once its secret query values are known.
fn mask(text: &str, secrets: &[&str]) -> String {
    let mut safe = text.to_owned();
    for secret in secrets {
        safe = safe.replace(secret, "***");
    }
    safe
}

/// Cookies kept apart by the exact origin that set them.
///
/// reqwest's own store follows the cookie specification, which scopes a cookie to a
/// **host** and deliberately does not isolate by port. Every service in this stack is
/// a different port on `127.0.0.1`, so that specification-correct behaviour hands
/// qBittorrent's session cookie to every other service on the machine: nineteen
/// third-party images receiving a live credential for the download client, on every
/// dashboard refresh and every queue read. Those images are not attackers, but they
/// are somebody else's code, and a credential that reaches them is a credential
/// outside this product's control.
///
/// So this keeps cookies by scheme, host **and** port, and returns one only to the
/// origin that set it. Deliberately narrower than the specification: nothing here
/// talks to a site that expects its cookies shared across its own ports, and the
/// cost of being wrong in that direction is a login that has to happen again.
///
/// The attributes after the first `;` of a `Set-Cookie` are not read. They govern
/// sharing and lifetime, and this store is already stricter than any of them would
/// ask for — it shares with nothing and lives no longer than the process.
#[derive(Debug, Default)]
pub struct PerOrigin {
    /// The cookies each origin set, by name, so a later value replaces an earlier one.
    held: Mutex<BTreeMap<String, BTreeMap<String, String>>>,
}

impl reqwest::cookie::CookieStore for PerOrigin {
    fn set_cookies(&self, headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        let pairs: Vec<(String, String)> = headers.filter_map(|header| pair(header)).collect();
        if let Ok(mut held) = self.held.lock() {
            held.entry(origin(url)).or_default().extend(pairs);
        }
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        let held = self.held.lock().ok()?;
        let jar = held.get(&origin(url))?;
        let said = jar.iter().fold(String::new(), |mut all, (name, value)| {
            if !all.is_empty() {
                all.push_str("; ");
            }
            all.push_str(name);
            all.push('=');
            all.push_str(value);
            all
        });
        HeaderValue::from_str(&said).ok()
    }
}

/// The name and value a `Set-Cookie` carries, where it carries a readable one.
fn pair(header: &HeaderValue) -> Option<(String, String)> {
    let said = header.to_str().ok()?;
    let (name, value) = said.split(';').next()?.split_once('=')?;
    Some((name.trim().to_owned(), value.trim().to_owned()))
}

/// Scheme, host and port together — what "the same service" means on a machine
/// running twenty of them behind one address.
fn origin(url: &Url) -> String {
    format!(
        "{}://{}:{}",
        url.scheme(),
        url.host_str().unwrap_or_default(),
        url.port_or_known_default().unwrap_or_default()
    )
}
