//! Proving a credential only where the operator allows the request that proves it.
//!
//! A decorator rather than a flag inside the live validator, for the reason the
//! retrying transport is one: what may be reached is a property of this machine's
//! settings, and the code that turns a credential into a request should not have to
//! know about them. Wrapping also means the refusal is in one place for both
//! credentials rather than at each of the two call sites that prove one.
//!
//! What comes back is `unreachable`, which is the outcome that already means *nothing
//! can be concluded about this credential* — and nothing can, because nothing was
//! asked. The detail says so in as many words rather than leaving somebody to go and
//! look at their network for a request that was never made.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Credential, Validation, Validator};
use crate::config::{Reaching, REACH_INDEXER_KEY, REACH_USENET_KEY};

/// A validator that proves only what this machine is allowed to reach.
pub struct Allowed {
    inner: Arc<dyn Validator>,
    reaching: Reaching,
}

impl Allowed {
    /// `inner`, held to what `reaching` permits.
    #[must_use]
    pub fn new(inner: Arc<dyn Validator>, reaching: Reaching) -> Self {
        Self { inner, reaching }
    }
}

/// What a credential comes to when the request that would prove it is switched off.
fn not_asked(switch: &str, service: &str) -> Validation {
    Validation::Unreachable {
        detail: format!(
            "nothing was asked: reaching {service} is switched off in {switch}, so this \
             credential is recorded as unverified rather than proven"
        ),
    }
}

#[async_trait]
impl Validator for Allowed {
    async fn validate(&self, credential: &Credential) -> Validation {
        validated(self, credential).await
    }
}

async fn validated(allowed: &Allowed, credential: &Credential) -> Validation {
    match credential {
        Credential::Indexer { .. } if !allowed.reaching.allows(REACH_INDEXER_KEY) => {
            not_asked(REACH_INDEXER_KEY, "the indexer")
        }
        Credential::Usenet { .. } if !allowed.reaching.allows(REACH_USENET_KEY) => {
            not_asked(REACH_USENET_KEY, "the Usenet provider")
        }
        // A service the operator asked lemonfiber to adopt is part of the stack
        // this product operates rather than a third party on the internet, and
        // reaching it is the same reach as every other call lemonfiber makes to
        // the services it manages. See `crate::outbound` for where that line is
        // drawn and why.
        other => allowed.inner.validate(other).await,
    }
}

#[cfg(test)]
mod tests;
