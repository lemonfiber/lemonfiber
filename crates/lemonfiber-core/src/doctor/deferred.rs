//! A check whose readings are made when it runs rather than when it is assembled.
//!
//! Some checks need something read before they can be built: what the download
//! clients still have to write, the port a client says it listens on, the accounts
//! the providers keep. Read while the register is assembled, those readings happen
//! for every check whether or not it is asked for — one category, or one check by
//! name, still waits for all of them — and outside every check's budget, so a client
//! that will not answer holds up the whole run with nothing to say why.
//!
//! So such a check is held as the building of it, with the family it belongs to and
//! the budget it runs under known up front. It is built the first time it runs, as
//! the first part of that run, and only a run that reaches it builds it.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;

use super::{Category, Check, Finding, Mend};

/// What builds a deferred check: its readings, and the check made from them.
type Building = Box<dyn Fn() -> Pin<Box<dyn Future<Output = Box<dyn Check>> + Send>> + Send + Sync>;

/// A check built the first time it runs.
pub(crate) struct Deferred {
    /// The family it belongs to, for narrowing before it is built.
    category: Category,
    /// How long building it and running it may take together.
    budget: Duration,
    /// What builds it.
    building: Building,
    /// The check, once a run has built it.
    built: tokio::sync::OnceCell<Box<dyn Check>>,
}

impl Deferred {
    /// A check in `category`, run within `budget`, built by `building` when it runs.
    pub(crate) fn new<Builds, Built>(category: Category, budget: Duration, building: Builds) -> Self
    where
        Builds: Fn() -> Built + Send + Sync + 'static,
        Built: Future<Output = Box<dyn Check>> + Send + 'static,
    {
        Self {
            category,
            budget,
            building: Box::new(move || Box::pin(building())),
            built: tokio::sync::OnceCell::new(),
        }
    }
}

#[async_trait]
impl Check for Deferred {
    fn category(&self) -> Category {
        self.category
    }

    fn budget(&self) -> Duration {
        self.budget
    }

    async fn run(&self) -> Vec<Finding> {
        settled(self).await.run().await
    }

    /// What the built check can put right, which there is only once a run built it:
    /// a mender is asked after the run that found what it would mend.
    fn mender(&self) -> Option<&dyn Mend> {
        self.built.get().and_then(|check| check.mender())
    }
}

impl Deferred {
    /// The check a run built, where one has.
    #[cfg(test)]
    pub(crate) fn built(&self) -> Option<&dyn Check> {
        self.built.get().map(AsRef::as_ref)
    }
}

/// The check, built here where no run has built it yet.
///
/// The one place it is settled: a run is what builds it, so every run asks here.
async fn settled(deferred: &Deferred) -> &dyn Check {
    deferred
        .built
        .get_or_init(|| (deferred.building)())
        .await
        .as_ref()
}

#[cfg(test)]
mod tests;
