use lemonfiber_manifest::Date;

use super::{Arc, Check, Indexers, ProvidersCheck, SystemTime, UsenetAccounts, Verdict};
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
    async fn indexers(&self, _now: SystemTime) -> Result<Vec<IndexerUse>, Failure> {
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
        limits: None,
        searched_from: None,
        grabbed_from: None,
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
        SystemTime::UNIX_EPOCH,
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
