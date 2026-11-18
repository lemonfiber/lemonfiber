//! Who an answer is worded for, and a member's answer in the household's words.
//!
//! The operator is answered with every word the core said, and a key that is no member's
//! with every word but what a run-and-exit service wrote, through [`Outcome::for_a_key`].
//! A household member's session,
//! and a key acting as one, is answered with the same envelope worded for the household:
//! an outcome through [`Outcome::for_household`], and a refusal through
//! [`lemonfiber_core::error::household::spoken`], the one place a member's wording is
//! chosen. A refusal carries its problem on the response, so the guard every route sits
//! behind words it for a member whichever route raised it.

use axum::http::StatusCode;
use axum::response::Response;
use lemonfiber_core::app::Outcome;
use lemonfiber_core::error::household;
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{kind, Envelope};

use crate::admission::{Caller, RETRY_AFTER};
use crate::read::enveloped;

/// Who an answer is worded for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    /// Whoever runs the house, or a program acting for them: every word as the core said it.
    Operator,
    /// A key that is no member's: every word but the lines a run-and-exit service wrote.
    Key,
    /// A household member's session, or a key acting as one: in the household's words.
    Household,
}

impl Audience {
    /// Who `caller` is answered as.
    #[must_use]
    pub fn of(caller: &Caller) -> Self {
        if caller.member().is_some() {
            Self::Household
        } else if matches!(caller, Caller::Key(_)) {
            Self::Key
        } else {
            Self::Operator
        }
    }

    /// `outcome` as this audience is answered with it.
    #[must_use]
    pub fn outcome(self, outcome: Outcome) -> Outcome {
        match self {
            Self::Operator => outcome,
            Self::Key => outcome.for_a_key(),
            Self::Household => outcome.for_household(),
        }
    }

    /// `problem` as this audience is answered with it.
    #[must_use]
    pub fn problem(self, problem: &Problem) -> Problem {
        match self {
            Self::Operator | Self::Key => problem.clone(),
            Self::Household => household::spoken(problem),
        }
    }
}

/// The problem a refusal was answered with, carried on its response.
#[derive(Debug, Clone)]
pub(crate) struct Refused(pub(crate) Problem);

/// A refusal at `status`, in the envelope every refusal arrives in, carrying its problem
/// and, where its code says how long to wait before trying again, `Retry-After`.
pub(crate) fn refused(status: StatusCode, problem: Problem) -> Response {
    let mut response = enveloped(status, Envelope::new(kind::ERROR, &problem).to_json());
    if let Some(seconds) = crate::read::retry_after(&problem) {
        response
            .headers_mut()
            .insert(RETRY_AFTER, axum::http::HeaderValue::from(seconds));
    }
    response.extensions_mut().insert(Refused(problem));
    response
}

/// `response` as `audience` is answered with it: a refusal it carries worded for the
/// household, and everything else as it is, the status and every header kept.
pub(crate) fn for_audience(audience: Audience, mut response: Response) -> Response {
    if audience != Audience::Household {
        return response;
    }
    let Some(Refused(problem)) = response.extensions_mut().remove::<Refused>() else {
        return response;
    };
    let spoken = household::spoken(&problem);
    let rebuilt = enveloped(
        response.status(),
        Envelope::new(kind::ERROR, &spoken).to_json(),
    );
    *response.body_mut() = rebuilt.into_body();
    response.extensions_mut().insert(Refused(spoken));
    response
}

#[cfg(test)]
mod tests;
