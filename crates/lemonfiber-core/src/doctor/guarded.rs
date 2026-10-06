//! Whether the Usenet indexer aggregator keeps the indexer accounts it holds to itself.
//!
//! The read-only half of the seeding step that turns its authentication on. Seeding
//! turns it on and keeps it on; this asks the same question changing nothing — whether a
//! read of the configuration presenting nothing is refused — so an operator running a
//! diagnosis is told when the accounts are open to anything that can reach the service,
//! whoever turned the authentication off — and when the service will not say, which is
//! never taken as guarded.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::codes::config::AGGREGATOR_EXPOSED;
use crate::error::{Problem, Remedy, Severity};
use crate::ports::service::Failure;

/// A service that can be asked whether it answers its configuration to anybody.
#[async_trait]
pub trait Exposure: Send + Sync {
    /// Whether a read of the configuration presenting nothing is answered.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the service could not be asked.
    async fn exposed(&self) -> Result<bool, Failure>;
}

#[async_trait]
impl Exposure for crate::nzbhydra2::Nzbhydra2 {
    async fn exposed(&self) -> Result<bool, Failure> {
        crate::nzbhydra2::Nzbhydra2::exposed(self).await
    }
}

/// The name this check and anything answering it share.
const CHECK: &str = "config.aggregator-guarded";

/// The heading an operator reads this under.
const TITLE: &str = "Who can read the indexer accounts";

/// Reports whether the Usenet indexer aggregator answers its configuration to anybody.
pub struct GuardedCheck {
    /// The aggregator and what it is called, absent where the stack runs none this
    /// machine can reach.
    aggregator: Option<(Arc<dyn Exposure>, String)>,
}

impl GuardedCheck {
    /// A check over the aggregator given, called `name`.
    #[must_use]
    pub fn new(aggregator: Option<(Arc<dyn Exposure>, String)>) -> Self {
        Self { aggregator }
    }
}

#[async_trait]
impl Check for GuardedCheck {
    fn category(&self) -> Category {
        Category::Config
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }
}

/// Whether the aggregator answers anybody, where there is one to ask.
async fn ran(check: &GuardedCheck) -> Vec<Finding> {
    let Some((client, name)) = check.aggregator.as_ref() else {
        return vec![finding(Verdict::Skipped {
            reason: "this stack runs no Usenet indexer aggregator this machine can reach"
                .to_owned(),
        })];
    };
    let verdict = match client.exposed().await {
        Ok(true) => Verdict::Warn(exposed(name)),
        Ok(false) => Verdict::Pass {
            note: Some(format!(
                "{name} refuses a read of its configuration to anybody presenting nothing"
            )),
        },
        // Not knowing is not a pass: an aggregator that will not say who it answers may
        // be answering anybody.
        Err(failure) => Verdict::Warn(unknown(name, &failure)),
    };
    vec![finding(verdict)]
}

/// The one finding this check produces, under the name anything answering it shares.
fn finding(verdict: Verdict) -> Finding {
    Finding::in_category(Category::Config, CHECK, TITLE, verdict)
}

/// An aggregator that would not say whether it answers its configuration to a caller
/// presenting nothing.
fn unknown(name: &str, failure: &Failure) -> Problem {
    Problem::new(
        AGGREGATOR_EXPOSED,
        Severity::Warning,
        format!("{name} may answer its configuration to anybody"),
        format!(
            "{name} could not be asked whether it answers its configuration, the indexer \
             accounts it holds and their keys among it, to anything that can reach it: \
             {failure}"
        ),
        Remedy::new(
            "Check the service is up and has finished starting, then run the diagnosis again",
        )
        .with_detail("lemonfiber status"),
    )
}

/// An aggregator that answers its configuration to a caller presenting nothing.
fn exposed(name: &str) -> Problem {
    Problem::new(
        AGGREGATOR_EXPOSED,
        Severity::Warning,
        format!("{name} answers its configuration to anybody"),
        crate::seed::run::exposure(name),
        Remedy::new(format!(
            "Run `lemonfiber seed`, or `lemonfiber reset` where you turned it off, to turn \
             {name}'s authentication on"
        )),
    )
}

#[cfg(test)]
mod tests;
