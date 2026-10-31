//! How a call crosses: what is asked, what is answered, and a refusal.
//!
//! A refusal is a problem document whose `type` is one the contract declares, so the core
//! turns it into its own answer by that word alone and never by reading the prose an
//! adapter wrote beside it. The five types are the five ways a port already fails
//! ([`Failure`]), plus two that only a contract can: a call nobody declared, and one whose
//! body did not read.

use std::any::Any;

use lemonfiber_ports::service::Failure;
use serde::{Deserialize, Serialize};

pub use lemonfiber_ports::http::{JSON, PROBLEM};

/// Why an adapter refused a call, as the contract declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The upstream did not answer.
    Unavailable,
    /// The upstream refused the credential the adapter holds for it.
    Unauthorised,
    /// The upstream answered in a way the adapter did not expect.
    Refused,
    /// The upstream speaks a release the adapter does not support, or cannot perform the
    /// operation at all, which is then not offered.
    Unsupported,
    /// The operation is not one this adapter serves.
    UnknownOperation,
    /// What was asked did not read as the operation's request.
    NotAsked,
    /// What was asked read as the operation's request, and is one the operation never
    /// does, as a challenge solver refuses a page inside the household.
    Disallowed,
}

impl Kind {
    /// The status a refusal of this kind is answered with.
    #[must_use]
    pub const fn status(self) -> u16 {
        match self {
            Self::Unavailable => 503,
            Self::Unauthorised => 502,
            Self::Refused | Self::Unsupported => 500,
            Self::UnknownOperation => 404,
            Self::NotAsked | Self::Disallowed => 400,
        }
    }
}

/// A refusal, as a problem document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    /// Which refusal it is.
    #[serde(rename = "type")]
    pub kind: Kind,
    /// What the adapter said, for a person reading a log. Never decided on.
    pub detail: String,
}

impl Refusal {
    /// A refusal of `kind`, with `detail` for a person.
    #[must_use]
    pub fn new(kind: Kind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    /// The refusal for an operation nobody declared.
    #[must_use]
    pub fn unknown_operation(operation: &str) -> Self {
        Self::new(
            Kind::UnknownOperation,
            format!("{operation} is not an operation this adapter serves"),
        )
    }

    /// The failure the core reads this refusal as, for the service `service`.
    #[must_use]
    pub fn failure(&self, service: &str) -> Failure {
        let service = service.to_owned();
        let detail = self.detail.clone();
        match self.kind {
            Kind::Unavailable => Failure::Unavailable { service },
            Kind::Unauthorised => Failure::Unauthorised { service },
            Kind::Unsupported => Failure::Unsupported { service, detail },
            Kind::Disallowed => Failure::Disallowed { service, detail },
            Kind::Refused | Kind::UnknownOperation | Kind::NotAsked => {
                Failure::Refused { service, detail }
            }
        }
    }
}

impl From<&Failure> for Refusal {
    fn from(failure: &Failure) -> Self {
        match failure {
            Failure::Unavailable { service } => {
                Self::new(Kind::Unavailable, format!("{service} is not answering"))
            }
            Failure::Unauthorised { service } => Self::new(
                Kind::Unauthorised,
                format!("{service} rejected the credential"),
            ),
            Failure::Refused { detail, .. } => Self::new(Kind::Refused, detail.clone()),
            Failure::Unsupported { detail, .. } => Self::new(Kind::Unsupported, detail.clone()),
            Failure::Disallowed { detail, .. } => Self::new(Kind::Disallowed, detail.clone()),
        }
    }
}

/// What a call asked, read from its body.
///
/// # Errors
///
/// [`Kind::NotAsked`] where the body does not read as `A`.
pub fn asked<A: serde::de::DeserializeOwned>(body: &[u8]) -> Result<A, Refusal> {
    serde_json::from_slice(body).map_err(|why| Refusal::new(Kind::NotAsked, why.to_string()))
}

/// The status an adapter answers an operation with that applied what it was asked and is
/// in force only once the upstream restarts.
pub const RESET: u16 = 205;

/// What an adapter answers a call it served with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// This JSON, at `200`.
    Json(Vec<u8>),
    /// Applied, and in force once the upstream restarts: [`RESET`], with nothing to say.
    Reset,
}

/// What a port answered, as what an adapter sends back.
///
/// # Errors
///
/// The refusal a failure crosses as.
pub fn answered<R: Serialize + 'static>(answer: Result<R, Failure>) -> Result<Answer, Refusal> {
    let answer = answer.map_err(|failure| Refusal::from(&failure))?;
    if lemonfiber_ports::service::on_restart(&answer as &dyn Any) {
        return Ok(Answer::Reset);
    }
    serde_json::to_vec(&answer)
        .map(Answer::Json)
        .map_err(|why| Refusal::new(Kind::Refused, why.to_string()))
}

#[cfg(test)]
mod tests;
