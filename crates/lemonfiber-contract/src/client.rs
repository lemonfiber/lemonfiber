//! Asking an adapter, and holding what it answers to the contract.
//!
//! Every call is a `POST` of what is asked to the operation's path, under the plugin's own
//! key. What comes back is read only if it is one of the answers the contract declares: a
//! success whose body decodes as the operation's answer, or a refusal whose body decodes
//! as one. Anything else, a status nobody declared, a body past the operation's bound
//! ([`LARGEST`] unless it declares more), an answer
//! after [`DEADLINE`], a field the type does not have, is refused, never read, and told
//! to the [`Witness`], so the plugin's standing records that it answered outside its
//! contract.

use std::sync::Arc;
use std::time::Duration;

use lemonfiber_ports::http::{Fetched, Http, Method, Request};
use lemonfiber_ports::service::Failure;

use crate::wire::{self, Refusal};

/// What an adapter's own account of itself is named in what is witnessed about it.
const ABOUT: &str = "about";

/// The most an answer may carry, in bytes, where its operation declares no more.
pub const LARGEST: usize = 1024 * 1024;

/// What a call made without the key carries: nothing an operation could read.
const UNKEYED_ASKED: &str = "{}";

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
        largest: usize,
        asked: &A,
    ) -> Result<R, Failure>
    where
        A: serde::Serialize + Sync,
        R: serde::de::DeserializeOwned,
    {
        let path = crate::path(capability, major, operation);
        let written = Some(serde_json::to_string(asked));
        let fetched = self
            .exchange(Method::Post, &path, written, largest, true)
            .await?;
        let answer = self.answer(operation, &fetched)?;
        serde_json::from_str(answer)
            .map_err(|why| self.outside(operation, &format!("the answer did not read: {why}")))
    }

    /// What the adapter says it is: the contracts it speaks and the upstream releases it
    /// supports.
    ///
    /// # Errors
    ///
    /// As [`Self::call`].
    pub async fn about(&self) -> Result<crate::adapter::About, Failure> {
        let path = crate::adapter::about_path();
        let fetched = self
            .exchange(Method::Get, &path, None, LARGEST, true)
            .await?;
        let answer = self.answer(ABOUT, &fetched)?;
        serde_json::from_str(answer)
            .map_err(|why| self.outside(ABOUT, &format!("the answer did not read: {why}")))
    }

    /// The status the adapter answers `operation` with when it is asked without the key.
    ///
    /// # Errors
    ///
    /// [`Failure::Unavailable`] where the adapter cannot be reached or does not answer in
    /// time.
    pub async fn unkeyed(&self, operation: &crate::Operation) -> Result<u16, Failure> {
        let asked = Some(Ok(UNKEYED_ASKED.to_owned()));
        self.exchange(Method::Post, &operation.path(), asked, LARGEST, false)
            .await
            .map(|fetched| fetched.status)
    }

    /// Send what was asked, written, and wait for the adapter's response.
    ///
    /// Everything a call decides before decoding is decided here and in [`Self::answer`],
    /// outside the generic [`Self::call`], so it is compiled and tested once rather than
    /// once per operation.
    async fn exchange(
        &self,
        method: Method,
        path: &str,
        written: Option<serde_json::Result<String>>,
        largest: usize,
        keyed: bool,
    ) -> Result<Fetched, Failure> {
        let body = written
            .transpose()
            .map_err(|why| self.refused(&why.to_string()))?;
        let key = keyed.then(|| ("Authorization".to_owned(), format!("Bearer {}", self.key)));
        let request = Request {
            method,
            url: format!("{}{path}", self.base.trim_end_matches('/')),
            headers: key
                .into_iter()
                .chain([
                    ("Content-Type".to_owned(), wire::JSON.to_owned()),
                    ("Accept".to_owned(), wire::JSON.to_owned()),
                ])
                .collect(),
            body,
            pinned: None,
        };
        let sent = tokio::time::timeout(DEADLINE, self.http.fetch(&request, largest)).await;
        let Ok(Ok(fetched)) = sent else {
            return Err(Failure::Unavailable {
                service: self.service.clone(),
            });
        };
        Ok(fetched)
    }

    /// The JSON one response answers with, if it is an answer the contract declares.
    fn answer<'r>(&self, operation: &str, fetched: &'r Fetched) -> Result<&'r str, Failure> {
        let Some(bytes) = fetched.bytes.as_deref() else {
            return Err(self.outside(operation, "the answer is larger than the contract allows"));
        };
        let Ok(body) = std::str::from_utf8(bytes) else {
            return Err(self.outside(operation, "the answer is not text"));
        };
        match fetched.status {
            200 => Ok(body),
            204 if body.is_empty() => Ok("null"),
            401 => Err(Failure::Unauthorised {
                service: self.service.clone(),
            }),
            status if is_problem(fetched) => match serde_json::from_str::<Refusal>(body) {
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
fn is_problem(fetched: &Fetched) -> bool {
    fetched
        .header("content-type")
        .is_some_and(|kind| kind.starts_with(wire::PROBLEM))
}

#[cfg(test)]
mod tests;
