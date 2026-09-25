//! Whether the accounts the stack depends on can still serve it.
//!
//! Every other check asks whether the software is working. This one asks whether the
//! third-party accounts underneath it are, because when one lapses the symptom is
//! indistinguishable from a broken installation: nothing downloads, every service is
//! green, and the operator restarts things that were never wrong.
//!
//! Nothing here spends any of what it measures. A Usenet account's allowance is read
//! from the download client that has been pulling from it, and an indexer's use from
//! the aggregator that has been querying it — both keep their own records, so asking
//! them how much has gone costs the provider nothing. A check that consumed the quota
//! it reports on would be a check that causes the outage it warns about.

mod indexers;
mod usenet;

use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use lemonfiber_manifest::Date;

use super::{Category, Check, Finding, Verdict};
use crate::error::codes::provider::{
    INDEXERS_ALL_FAILING, INDEXER_CAPPED, INDEXER_RESTED, PROVIDER_CROWDED, PROVIDER_EMPTY,
    PROVIDER_ENDING, PROVIDER_LOW, PROVIDER_REFUSED, PROVIDER_SILENT,
};
use crate::error::Remedy;
use crate::ports::service::{Failure, Indexers, UsenetAccounts};

/// Reports on the accounts behind the stack: what they have left, and whether they
/// are still serving it.
pub struct ProvidersCheck {
    accounts: Option<Arc<dyn UsenetAccounts>>,
    indexers: Option<Arc<dyn Indexers>>,
    today: Date,
    now: SystemTime,
}

impl ProvidersCheck {
    /// A check over whichever of the two the stack has — a torrent-only stack has no
    /// Usenet accounts to read, and a stack whose aggregator is not up yet has no
    /// indexers, and neither is a fault.
    ///
    /// The day and the moment are both taken because the two sides measure in different
    /// units and neither can be had from the other here: a download client keeps its
    /// figures per calendar day, and an aggregator counts against a window that rolls.
    #[must_use]
    pub fn new(
        accounts: Option<Arc<dyn UsenetAccounts>>,
        indexers: Option<Arc<dyn Indexers>>,
        today: Date,
        now: SystemTime,
    ) -> Self {
        Self {
            accounts,
            indexers,
            today,
            now,
        }
    }

    /// What the download client says about the Usenet accounts behind it.
    ///
    /// A disabled account is left out: the client is not pulling through it, so
    /// nothing about its allowance is a fault the operator has to act on.
    async fn usenet(&self) -> Vec<Finding> {
        let Some(client) = &self.accounts else {
            return Vec::new();
        };
        match client.accounts().await {
            Err(failure) => vec![unread("providers.usenet", "Usenet accounts", &failure)],
            Ok(accounts) => accounts
                .iter()
                .filter(|account| account.enabled)
                .flat_map(|account| usenet::findings(account, self.today))
                .collect(),
        }
    }

    /// What the aggregator says about the indexers it queries.
    async fn indexers(&self) -> Vec<Finding> {
        let Some(aggregator) = &self.indexers else {
            return Vec::new();
        };
        match aggregator.indexers(self.now).await {
            Err(failure) => vec![unread("providers.indexers", "Indexers", &failure)],
            Ok(listed) => indexers::findings(&listed),
        }
    }
}

#[async_trait]
impl Check for ProvidersCheck {
    fn category(&self) -> Category {
        Category::Providers
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }
}

/// Whether each declared provider is configured and reachable.
async fn ran(check: &ProvidersCheck) -> Vec<Finding> {
    let mut findings = check.usenet().await;
    findings.extend(check.indexers().await);
    if findings.is_empty() {
        findings.push(Finding::in_category(
            Category::Providers,
            "providers",
            "Providers",
            Verdict::Skipped {
                reason: "there are no accounts in use to read — no Usenet account the download client is pulling through, and no indexer being queried".to_owned(),
            },
        ));
    }
    findings
}

/// A source that could not be read at all.
///
/// Unverified rather than a failure: a client that will not answer says nothing about
/// whether the accounts behind it are healthy, and reporting silence as a healthy
/// account is how an operator comes to trust a figure nobody measured.
fn unread(check: &str, title: &str, failure: &Failure) -> Finding {
    Finding::in_category(
        Category::Providers,
        check,
        title,
        Verdict::Unverified {
            reason: format!("{failure}, so what the accounts have left could not be read"),
            remedy: Remedy::new("Check the service is running, then run the check again"),
        },
    )
}

#[cfg(test)]
mod tests;
