//! An upstream reached over the HTTP port, and what to call it when something goes wrong:
//! a transport failure as the upstream not answering, a refused credential as one, and any
//! other answer that cannot be used as a refusal in the upstream's own words, withheld and
//! shortened.

use std::sync::Arc;

use lemonfiber_ports::http::{Http, Request, Response};
use lemonfiber_ports::service::Failure;

/// The most of an upstream's own words a refusal carries.
const DETAIL_LIMIT: usize = 200;

/// What stands where the middle of a long refusal was cut.
const MARKER: &str = "...";

/// The fields of a JSON answer an upstream says what went wrong in.
const SAID_IN: [&str; 2] = ["message", "error"];

/// Render form fields as an `application/x-www-form-urlencoded` query.
#[must_use]
pub fn form_encoded(fields: &[(&str, &str)]) -> String {
    let mut form = form_urlencoded::Serializer::new(String::new());
    for (name, value) in fields {
        form.append_pair(name, value);
    }
    form.finish()
}

/// A service reached over the HTTP port, and what to call it when something goes wrong.
pub struct Endpoint {
    http: Arc<dyn Http>,
    base: String,
    service: String,
}

impl Endpoint {
    /// An endpoint for `service`, reached at `base` over `http`.
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, service: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into(),
            service: service.into(),
        }
    }

    /// A URL for `path` under the base, the base's trailing slash dropped.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base.trim_end_matches('/'))
    }

    /// Send a request, a transport failure read as the service not answering.
    ///
    /// # Errors
    ///
    /// [`Failure::Unavailable`] where nothing answered.
    pub async fn send(&self, request: &Request) -> Result<Response, Failure> {
        self.http
            .send(request)
            .await
            .map_err(|_| self.unavailable())
    }

    /// The upstream not answering.
    fn unavailable(&self) -> Failure {
        Failure::Unavailable {
            service: self.service.clone(),
        }
    }

    /// A non-success response: a refused credential, a service not answering yet, or an
    /// answer that cannot be used.
    #[must_use]
    pub fn refusal(&self, response: &Response) -> Failure {
        match response.status {
            401 | 403 => self.unauthorised(),
            503 => self.unavailable(),
            _ => self.refused(&describe(response)),
        }
    }

    /// A rejected credential, blamed on the service.
    #[must_use]
    pub fn unauthorised(&self) -> Failure {
        Failure::Unauthorised {
            service: self.service.clone(),
        }
    }

    /// An answer that arrived and could not be used, for `detail`.
    #[must_use]
    pub fn refused(&self, detail: &str) -> Failure {
        Failure::Refused {
            service: self.service.clone(),
            detail: detail.to_owned(),
        }
    }

    /// Nothing where the response succeeded, the refusal its status amounts to otherwise.
    ///
    /// # Errors
    ///
    /// The refusal a status other than a success amounts to.
    pub fn expect_success(&self, response: &Response) -> Result<(), Failure> {
        if response.is_success() {
            Ok(())
        } else {
            Err(self.refusal(response))
        }
    }

    /// A JSON body read as `T`, a body that does not parse named by `what`.
    ///
    /// # Errors
    ///
    /// The refusal a status other than a success amounts to, or one naming `what` where
    /// the body is not a `T`.
    pub fn decode<T: serde::de::DeserializeOwned>(
        &self,
        response: &Response,
        what: &str,
    ) -> Result<T, Failure> {
        if !response.is_success() {
            return Err(self.refusal(response));
        }
        serde_json::from_str(&response.body).map_err(|err| {
            self.refused(&format!(
                "{what}: the answer is not in the shape expected at line {} column {}",
                err.line(),
                err.column()
            ))
        })
    }
}

/// A non-success response as a refusal's detail: the service's own words, withheld and
/// then shortened, or its status alone where it said none. From a JSON answer only the
/// words it says under `message` or `error` are taken, never the fields beside them.
#[must_use]
pub fn describe(response: &Response) -> String {
    let said = match serde_json::from_str::<serde_json::Value>(&response.body) {
        Ok(answer) => SAID_IN
            .iter()
            .find_map(|field| answer.get(field).and_then(serde_json::Value::as_str))
            .unwrap_or_default()
            .to_owned(),
        Err(_) => response.body.clone(),
    };
    let body = lemonfiber_error::withheld::withheld_text(said.trim());
    let body = body.trim();
    if body.is_empty() {
        format!("HTTP {}", response.status)
    } else {
        format!("HTTP {}: {}", response.status, fitted(body, DETAIL_LIMIT))
    }
}

/// `text` cut to `width` characters around its middle.
fn fitted(text: &str, width: usize) -> String {
    let counted = text.chars().count();
    if counted <= width {
        return text.to_owned();
    }
    let keep = width.saturating_sub(MARKER.len());
    let tail = keep / 2;
    let head = keep - tail;
    let front: String = text.chars().take(head).collect();
    let back: String = text.chars().skip(counted - tail).collect();
    format!("{front}{MARKER}{back}")
}

#[cfg(test)]
mod tests;
