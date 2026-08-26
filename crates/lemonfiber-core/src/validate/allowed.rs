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
        match credential {
            Credential::Indexer { .. } if !self.reaching.allows(REACH_INDEXER_KEY) => {
                not_asked(REACH_INDEXER_KEY, "the indexer")
            }
            Credential::Usenet { .. } if !self.reaching.allows(REACH_USENET_KEY) => {
                not_asked(REACH_USENET_KEY, "the Usenet provider")
            }
            // A service the operator asked lemonfiber to adopt is part of the stack
            // this product operates rather than a third party on the internet, and
            // reaching it is the same reach as every other call lemonfiber makes to
            // the services it manages. See `crate::outbound` for where that line is
            // drawn and why.
            other => self.inner.validate(other).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::Allowed;
    use crate::config::{Reaching, REACH_INDEXER_KEY, REACH_USENET_KEY};
    use crate::validate::{Credential, Validation, Validator};

    /// A validator that records what it was asked and says everything is fine, so a
    /// test can tell "refused" from "asked and the answer discarded".
    #[derive(Default)]
    struct Asked(Mutex<Vec<Credential>>);

    #[async_trait]
    impl Validator for Asked {
        async fn validate(&self, credential: &Credential) -> Validation {
            if let Ok(mut seen) = self.0.lock() {
                seen.push(credential.clone());
            }
            Validation::Valid {
                observed: "answered".to_owned(),
            }
        }
    }

    impl Asked {
        fn count(&self) -> usize {
            self.0.lock().map(|seen| seen.len()).unwrap_or_default()
        }
    }

    fn an_indexer() -> Credential {
        Credential::Indexer {
            url: "https://indexer.example/api".to_owned(),
            key: "k".repeat(20),
        }
    }

    fn a_provider() -> Credential {
        Credential::Usenet {
            host: "news.example.net".to_owned(),
            port: 563,
            secure: true,
            user: "someone".to_owned(),
            pass: "p".repeat(20),
        }
    }

    fn a_service() -> Credential {
        Credential::Service {
            url: "http://127.0.0.1:8989/api/v3/system/status".to_owned(),
            key: "k".repeat(20),
        }
    }

    #[tokio::test]
    async fn a_credential_the_operator_allows_is_proven_as_it_always_was() {
        let inner = Arc::new(Asked::default());
        let allowed = Allowed::new(inner.clone(), Reaching::default());
        for credential in [an_indexer(), a_provider(), a_service()] {
            assert!(matches!(
                allowed.validate(&credential).await,
                Validation::Valid { .. }
            ));
        }
        assert_eq!(inner.count(), 3);
    }

    #[tokio::test]
    async fn an_indexer_the_operator_refuses_is_never_asked() {
        let inner = Arc::new(Asked::default());
        let outcome = Allowed::new(inner.clone(), Reaching::without(REACH_INDEXER_KEY))
            .validate(&an_indexer())
            .await;
        assert!(
            matches!(&outcome, Validation::Unreachable { detail }
                if detail.contains(REACH_INDEXER_KEY) && detail.contains("unverified")),
            "{outcome:?}"
        );
        assert_eq!(inner.count(), 0, "the indexer was asked anyway");
    }

    #[tokio::test]
    async fn a_provider_the_operator_refuses_is_never_asked() {
        let inner = Arc::new(Asked::default());
        let outcome = Allowed::new(inner.clone(), Reaching::without(REACH_USENET_KEY))
            .validate(&a_provider())
            .await;
        assert!(
            matches!(&outcome, Validation::Unreachable { detail }
                if detail.contains(REACH_USENET_KEY)),
            "{outcome:?}"
        );
        assert_eq!(inner.count(), 0, "the provider was asked anyway");
    }

    /// One switch refuses one credential. A machine that stopped proving a Usenet
    /// login because an indexer was switched off would be a switch that means
    /// something other than what it says.
    #[tokio::test]
    async fn one_switch_refuses_one_credential_and_leaves_the_others_alone() {
        let inner = Arc::new(Asked::default());
        let allowed = Allowed::new(inner.clone(), Reaching::without(REACH_INDEXER_KEY));
        for credential in [a_provider(), a_service()] {
            assert!(matches!(
                allowed.validate(&credential).await,
                Validation::Valid { .. }
            ));
        }
        assert_eq!(inner.count(), 2);
    }

    #[tokio::test]
    async fn a_machine_that_refuses_everything_asks_nothing_of_either() {
        let inner = Arc::new(Asked::default());
        let allowed = Allowed::new(inner.clone(), Reaching::none());
        for credential in [an_indexer(), a_provider()] {
            assert!(matches!(
                allowed.validate(&credential).await,
                Validation::Unreachable { .. }
            ));
        }
        assert_eq!(inner.count(), 0);
    }
}
