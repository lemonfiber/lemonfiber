//! How a call crosses: what is asked, what is answered, and a refusal.
//!
//! A refusal is a problem document whose `type` is one the contract declares, so the core
//! turns it into its own answer by that word alone and never by reading the prose an
//! adapter wrote beside it. The four types are the four ways a port already fails
//! ([`Failure`]), plus two that only a contract can: a call nobody declared, and one whose
//! body did not read.

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
    /// The upstream speaks a release the adapter does not support.
    Unsupported,
    /// The operation is not one this adapter serves.
    UnknownOperation,
    /// What was asked did not read as the operation's request.
    NotAsked,
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
            Self::NotAsked => 400,
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
            format!("`{operation}` is not an operation this adapter serves"),
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
                Self::new(Kind::Unavailable, format!("`{service}` is not answering"))
            }
            Failure::Unauthorised { service } => Self::new(
                Kind::Unauthorised,
                format!("`{service}` rejected the credential"),
            ),
            Failure::Refused { detail, .. } => Self::new(Kind::Refused, detail.clone()),
            Failure::Unsupported { detail, .. } => Self::new(Kind::Unsupported, detail.clone()),
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

/// What a port answered, as the body an adapter sends back.
///
/// # Errors
///
/// The refusal a failure crosses as.
pub fn answered<R: Serialize>(answer: Result<R, Failure>) -> Result<Vec<u8>, Refusal> {
    let answer = answer.map_err(|failure| Refusal::from(&failure))?;
    serde_json::to_vec(&answer).map_err(|why| Refusal::new(Kind::Refused, why.to_string()))
}

#[cfg(test)]
mod tests;
