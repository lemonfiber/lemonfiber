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

use async_trait::async_trait;
use lemonfiber_manifest::Date;

use super::{Category, Check, Finding, Verdict};
use crate::error::{Code, Remedy};
use crate::ports::service::{Failure, Indexers, UsenetAccounts};

/// Raised when an account has nothing left to serve.
pub const PROVIDER_EMPTY: Code = Code::new("PROVIDER-1");

/// Raised when an account is running out, with time left to act.
pub const PROVIDER_LOW: Code = Code::new("PROVIDER-2");

/// Raised when the subscription behind an account ends soon.
pub const PROVIDER_ENDING: Code = Code::new("PROVIDER-3");

/// Raised when an account refuses the credential the client offers it.
pub const PROVIDER_REFUSED: Code = Code::new("PROVIDER-6");

/// Raised when an account has stopped answering the client entirely.
pub const PROVIDER_SILENT: Code = Code::new("PROVIDER-7");

/// Raised when the client is set to open more connections than an account allows.
pub const PROVIDER_CROWDED: Code = Code::new("PROVIDER-8");

/// Raised when an indexer has been failing and its aggregator has rested it.
pub const INDEXER_RESTED: Code = Code::new("PROVIDER-4");

/// Raised when every indexer is failing at once.
pub const INDEXERS_ALL_FAILING: Code = Code::new("PROVIDER-5");

/// Reports on the accounts behind the stack: what they have left, and whether they
/// are still serving it.
pub struct ProvidersCheck {
    accounts: Option<Arc<dyn UsenetAccounts>>,
    indexers: Option<Arc<dyn Indexers>>,
    today: Date,
}

impl ProvidersCheck {
    /// A check over whichever of the two the stack has — a torrent-only stack has no
    /// Usenet accounts to read, and a stack whose aggregator is not up yet has no
    /// indexers, and neither is a fault.
    #[must_use]
    pub fn new(
        accounts: Option<Arc<dyn UsenetAccounts>>,
        indexers: Option<Arc<dyn Indexers>>,
        today: Date,
    ) -> Self {
        Self {
            accounts,
            indexers,
            today,
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
            Err(failure) => vec![unread("usenet", "Usenet accounts", &failure)],
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
        match aggregator.indexers(self.today).await {
            Err(failure) => vec![unread("indexers", "Indexers", &failure)],
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
        let mut findings = self.usenet().await;
        findings.extend(self.indexers().await);
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
}

/// A source that could not be read at all.
///
/// Unverified rather than a failure: a client that will not answer says nothing about
/// whether the accounts behind it are healthy, and reporting silence as a healthy
/// account is how an operator comes to trust a figure nobody measured.
fn unread(slug: &str, title: &str, failure: &Failure) -> Finding {
    Finding::in_category(
        Category::Providers,
        &format!("providers.{slug}"),
        title,
        Verdict::Unverified {
            reason: format!("{failure}, so what the accounts have left could not be read"),
            remedy: Remedy::new("Check the service is running, then run the check again"),
        },
    )
}

#[cfg(test)]
mod tests {
    use lemonfiber_manifest::Date;

    use super::{Arc, Check, Indexers, ProvidersCheck, UsenetAccounts, Verdict};
    use crate::ports::service::{Failure, IndexerUse, Recorded, UsenetAccount};

    /// A client that answers with what it was given.
    struct Client(Result<Vec<UsenetAccount>, Failure>);

    #[async_trait::async_trait]
    impl UsenetAccounts for Client {
        async fn accounts(&self) -> Result<Vec<UsenetAccount>, Failure> {
            match &self.0 {
                Ok(accounts) => Ok(accounts.clone()),
                Err(_) => Err(Failure::Unavailable {
                    service: "sabnzbd".to_owned(),
                }),
            }
        }
    }

    /// An aggregator that answers with what it was given.
    struct Aggregator(Result<Vec<IndexerUse>, Failure>);

    #[async_trait::async_trait]
    impl Indexers for Aggregator {
        async fn indexers(&self, _since: Date) -> Result<Vec<IndexerUse>, Failure> {
            match &self.0 {
                Ok(indexers) => Ok(indexers.clone()),
                Err(_) => Err(Failure::Unavailable {
                    service: "prowlarr".to_owned(),
                }),
            }
        }
    }

    const fn today() -> Date {
        Date {
            year: 2026,
            month: 8,
            day: 16,
        }
    }

    fn account() -> UsenetAccount {
        UsenetAccount {
            name: "Block 500".to_owned(),
            enabled: true,
            quota: Some(Recorded {
                cap: 100 * (1 << 30),
                from: 0,
            }),
            downloaded: 0,
            daily: Vec::new(),
            expires_on: None,
            standing: None,
        }
    }

    fn indexer() -> IndexerUse {
        IndexerUse {
            name: "Fast".to_owned(),
            enabled: true,
            queries: 5,
            failed_queries: 0,
            grabs: 1,
            failed_grabs: 0,
            rested_until: None,
        }
    }

    fn checking(
        accounts: Option<Result<Vec<UsenetAccount>, Failure>>,
        indexers: Option<Result<Vec<IndexerUse>, Failure>>,
    ) -> ProvidersCheck {
        ProvidersCheck::new(
            accounts.map(|answer| Arc::new(Client(answer)) as Arc<dyn UsenetAccounts>),
            indexers.map(|answer| Arc::new(Aggregator(answer)) as Arc<dyn Indexers>),
            today(),
        )
    }

    #[tokio::test]
    async fn it_reports_in_the_providers_category() {
        assert_eq!(checking(None, None).category(), super::Category::Providers);
    }

    #[tokio::test]
    async fn a_stack_with_neither_source_has_nothing_to_read_rather_than_nothing_wrong() {
        let findings = checking(None, None).run().await;
        assert!(matches!(
            findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Skipped { .. })
        ));
    }

    #[tokio::test]
    async fn both_sources_are_reported_together() {
        let findings = checking(Some(Ok(vec![account()])), Some(Ok(vec![indexer()])))
            .run()
            .await;
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().any(|finding| finding.title == "Block 500"));
        assert!(findings.iter().any(|finding| finding.title == "Fast"));
    }

    /// A provider taken out of the client is gone from the report the moment it is
    /// gone from the client — every finding here is derived from what the services
    /// hold now, so there is no list of its own to fall out of step with them. What
    /// clears the condition behind it is that nothing raises it on the next pass.
    #[tokio::test]
    async fn a_provider_that_has_been_removed_stops_being_reported_on() {
        let before = checking(Some(Ok(vec![account()])), Some(Ok(vec![indexer()])))
            .run()
            .await;
        assert!(before.iter().any(|finding| finding.title == "Block 500"));

        let after = checking(Some(Ok(Vec::new())), Some(Ok(vec![indexer()])))
            .run()
            .await;
        assert!(!after.iter().any(|finding| finding.title == "Block 500"));
        assert_eq!(
            after.len(),
            1,
            "the indexer it was read beside still reports"
        );
    }

    /// A disabled account is not being pulled through, so nothing about its allowance
    /// is a fault waiting to happen.
    #[tokio::test]
    async fn an_account_the_client_is_not_using_is_left_out() {
        let switched_off = UsenetAccount {
            enabled: false,
            ..account()
        };
        let findings = checking(Some(Ok(vec![switched_off])), None).run().await;
        assert!(matches!(
            findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Skipped { .. }),
        ));
    }

    /// Silence is not health: a client that will not answer says nothing about the
    /// accounts behind it, and a check that read that as "fine" would be the same
    /// comfortable falsehood the trust checks exist to remove.
    #[tokio::test]
    async fn a_source_that_will_not_answer_is_unverified_rather_than_passing() {
        let findings = checking(
            Some(Err(Failure::Unavailable {
                service: "sabnzbd".to_owned(),
            })),
            Some(Err(Failure::Unavailable {
                service: "prowlarr".to_owned(),
            })),
        )
        .run()
        .await;
        assert_eq!(findings.len(), 2);
        assert!(findings
            .iter()
            .all(|finding| matches!(finding.verdict, Verdict::Unverified { .. })));
        assert!(findings
            .iter()
            .any(|finding| finding.check == "providers.usenet"));
        assert!(findings
            .iter()
            .any(|finding| finding.check == "providers.indexers"));
    }
}
