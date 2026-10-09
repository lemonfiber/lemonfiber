//! Asking an adapter, and holding what it answers to the contract.
//!
//! Every call is a `POST` of what is asked to the operation's path, under the plugin's own
//! key. What comes back is read only if it is one of the answers the contract declares: a
//! success whose body decodes as the operation's answer, or a refusal whose body decodes
//! as one. Anything else, a status nobody declared, a body past [`LARGEST`], an answer
//! after [`DEADLINE`], a field the type does not have, is refused, never read, and told
//! to the [`Witness`], so the plugin's standing records that it answered outside its
//! contract.

use std::sync::Arc;
use std::time::Duration;

use lemonfiber_ports::http::{Http, Method, Request};
use lemonfiber_ports::service::Failure;

use crate::wire::{self, Refusal};

/// The most an answer may carry, in bytes.
pub const LARGEST: usize = 1024 * 1024;

/// How long an answer may take.
pub const DEADLINE: Duration = Duration::from_secs(15);

/// Who is told when an adapter answers outside its contract.
pub trait Witness: Send + Sync {
    /// The adapter answered `operation` outside its contract, for `why`.
    fn nonconforming(&self, operation: &str, why: &str);
}

/// Told nothing: for a client nothing keeps standing for.
struct Unwitnessed;

impl Witness for Unwitnessed {
    fn nonconforming(&self, _operation: &str, _why: &str) {}
}

/// An adapter the core asks, as the service the manifest names it.
#[derive(Clone)]
pub struct Contracted {
    http: Arc<dyn Http>,
    base: String,
    key: String,
    service: String,
    witness: Arc<dyn Witness>,
}

impl std::fmt::Debug for Contracted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Contracted")
            .field("base", &self.base)
            .field("service", &self.service)
            .finish_non_exhaustive()
    }
}

impl Contracted {
    /// The adapter at `base`, named `service`, asked under `key`.
    ///
    /// `http` is expected to stop reading an answer at [`LARGEST`], as the adapters'
    /// transport does when it is `limited`, so a stranger's body is never read whole
    /// before it is refused. The check here holds whatever transport is given.
    #[must_use]
    pub fn new(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        service: impl Into<String>,
        key: impl Into<String>,
    ) -> Self {
        Self {
            http,
            base: base.into(),
            key: key.into(),
            service: service.into(),
            witness: Arc::new(Unwitnessed),
        }
    }

    /// The same adapter, with what it answers outside its contract told to `witness`.
    #[must_use]
    pub fn witnessed_by(mut self, witness: Arc<dyn Witness>) -> Self {
        self.witness = witness;
        self
    }

    /// The service the manifest names this adapter.
    #[must_use]
    pub fn service(&self) -> &str {
        &self.service
    }

    /// Ask `operation` of `capability`'s `major`, and read the answer as `R`.
    ///
    /// # Errors
    ///
    /// [`Failure::Unavailable`] where the adapter cannot be reached or does not answer in
    /// time; the failure a declared refusal names; [`Failure::Unauthorised`] where the
    /// adapter refused the key; [`Failure::Refused`] for an answer outside the contract.
    pub async fn call<A, R>(
        &self,
        capability: &str,
        major: u32,
        operation: &str,
        asked: &A,
    ) -> Result<R, Failure>
    where
        A: serde::Serialize + Sync,
        R: serde::de::DeserializeOwned,
    {
        let body = serde_json::to_string(asked).map_err(|why| self.refused(&why.to_string()))?;
        let request = Request {
            method: Method::Post,
            url: format!(
                "{}{}",
                self.base.trim_end_matches('/'),
                crate::path(capability, major, operation)
            ),
            headers: vec![
                ("Authorization".to_owned(), format!("Bearer {}", self.key)),
                ("Content-Type".to_owned(), wire::JSON.to_owned()),
                ("Accept".to_owned(), wire::JSON.to_owned()),
            ],
            body: Some(body),
            pinned: None,
        };
        let sent = tokio::time::timeout(DEADLINE, self.http.send(&request)).await;
        let Ok(Ok(response)) = sent else {
            return Err(Failure::Unavailable {
                service: self.service.clone(),
            });
        };
        self.read(operation, &response)
    }

    /// What one response answers, if it is an answer the contract declares.
    fn read<R: serde::de::DeserializeOwned>(
        &self,
        operation: &str,
        response: &lemonfiber_ports::http::Response,
    ) -> Result<R, Failure> {
        if response.body.len() > LARGEST {
            return Err(self.outside(operation, "the answer is larger than the contract allows"));
        }
        let decoded = |body: &str| {
            serde_json::from_str(body)
                .map_err(|why| self.outside(operation, &format!("the answer did not read: {why}")))
        };
        match response.status {
            200 => decoded(&response.body),
            // Only an answer with nothing to say may be empty, and it reads as nothing.
            204 if response.body.is_empty() => decoded("null"),
            401 => Err(Failure::Unauthorised {
                service: self.service.clone(),
            }),
            status if is_problem(response) => match serde_json::from_str::<Refusal>(&response.body)
            {
                Ok(refusal) if refusal.kind.status() == status => {
                    Err(refusal.failure(&self.service))
                }
                _ => Err(self.outside(operation, &format!("a refusal at {status} did not read"))),
            },
            status => Err(self.outside(
                operation,
                &format!("{status} is not an answer the contract declares"),
            )),
        }
    }

    /// An answer outside the contract: told to the witness, and refused.
    fn outside(&self, operation: &str, why: &str) -> Failure {
        self.witness.nonconforming(operation, why);
        self.refused(why)
    }

    /// A refusal blamed on this adapter.
    fn refused(&self, why: &str) -> Failure {
        Failure::Refused {
            service: self.service.clone(),
            detail: why.to_owned(),
        }
    }
}

/// Whether a response declares itself a problem document.
fn is_problem(response: &lemonfiber_ports::http::Response) -> bool {
    response
        .header("content-type")
        .is_some_and(|kind| kind.starts_with(wire::PROBLEM))
}

#[cfg(test)]
mod tests;
