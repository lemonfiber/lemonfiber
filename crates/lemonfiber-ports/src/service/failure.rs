//! Why a service would not answer, and what that means for the operator.
//!
//! Four causes told apart because their remedies differ — a wrong key, a service that is
//! not up, one that refused, and one speaking a version this build does not.

/// What an operator types to be offered the repairs.
///
/// Named once. It appears in a finding's remedy, in a report that offered something and did
/// nothing, and in the problem raised when a service's credential no longer works — and a
/// renamed flag that only two of the three learned about would send somebody to a command
/// that does not exist.
///
/// Here rather than beside the repairing model, because this is the lowest place that needs
/// it: a port telling an operator what to type must not have to reach up into the layer that
/// acts on what they type.
pub const ASK_FOR_REPAIRS: &str = "lemonfiber doctor --fix";

use super::{Code, Diagnose, Problem, Remedy, Severity, State};
use thiserror::Error;

/// A service refused, or could not be reached.
#[derive(Debug, Error)]
pub enum Failure {
    /// The service is not answering yet.
    #[error("`{service}` is not answering")]
    Unavailable {
        /// The service that was asked.
        service: String,
    },
    /// The service answered, and refused the credential.
    #[error("`{service}` rejected the credential")]
    Unauthorised {
        /// The service that refused.
        service: String,
    },
    /// The service answered with something unexpected.
    #[error("`{service}` answered unexpectedly: {detail}")]
    Refused {
        /// The service that answered.
        service: String,
        /// The service's own words.
        detail: String,
    },
    /// The service answered, but does not serve the API version lemonfiber speaks
    /// — it has been upgraded past it (or is older than it). Writing to it would
    /// mean writing something malformed, so it is reported instead.
    #[error("`{service}` does not serve the API version this build speaks: {detail}")]
    Unsupported {
        /// The service that answered.
        service: String,
        /// Which version is missing, in lemonfiber's words.
        detail: String,
    },
}

impl Diagnose for Failure {
    fn problem(&self) -> Problem {
        match self {
            Self::Unavailable { service } => Problem::new(
                SERVICE_UNAVAILABLE,
                Severity::Warning,
                format!("{service} was not ready, so it was skipped"),
                "Wiring is resumable. Nothing was changed for this service, and running seed again picks it up once it is answering.",
                Remedy::new("Wait for it to finish starting, then run seed again")
                    .with_detail("lemonfiber seed"),
            ),
            Self::Unauthorised { service } => Problem::new(
                SERVICE_UNAUTHORISED,
                Severity::Error,
                format!("{service} rejected the credential"),
                "The credential lemonfiber holds no longer matches the one the service expects, usually because it was changed in the service's own interface.",
                Remedy::new("Re-read the service's credential").with_detail(ASK_FOR_REPAIRS),
            )
            .in_state(State::Remediable),
            Self::Refused { service, detail } => Problem::unknown(
                SERVICE_REFUSED,
                Severity::Error,
                format!("{service} answered in a way lemonfiber did not expect"),
                "This is not a failure lemonfiber recognises, so it will not guess at what would fix it.",
            )
            .with_detail(detail.clone()),
            Self::Unsupported { service, detail } => Problem::new(
                SERVICE_UNSUPPORTED,
                Severity::Error,
                format!("{service} does not serve the API version this build speaks"),
                "The service was upgraded past — or stands before — the API version lemonfiber knows how to speak, so writing to it would mean writing something malformed. Nothing was changed for it.",
                Remedy::new("Match the service to the version lemonfiber supports, or update lemonfiber, then run seed again")
                    .with_detail("lemonfiber seed"),
            )
            .with_detail(detail.clone()),
        }
    }
}

/// Raised when a service is not answering yet.
pub const SERVICE_UNAVAILABLE: Code = Code::new("SEED-1");

/// Raised when a service rejects the credential lemonfiber holds.
pub const SERVICE_UNAUTHORISED: Code = Code::new("SEED-2");

/// Raised when a service answers with something unusable.
pub const SERVICE_REFUSED: Code = Code::new("SEED-3");

/// Raised when a service does not serve the API version this build speaks.
pub const SERVICE_UNSUPPORTED: Code = Code::new("SEED-4");
