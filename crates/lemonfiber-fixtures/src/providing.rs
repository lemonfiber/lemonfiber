//! A download client's usenet accounts and an aggregator's indexers, answered as a test
//! gives them.

use std::time::SystemTime;

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, IndexerUse, Indexers, UsenetAccount, UsenetAccounts};

/// A download client answering with the accounts it was given, or as one not answering
/// where it was given a failure.
pub struct Accounts(Result<Vec<UsenetAccount>, Failure>);

impl Accounts {
    /// A client answering as `answer` says.
    #[must_use]
    pub const fn answering(answer: Result<Vec<UsenetAccount>, Failure>) -> Self {
        Self(answer)
    }
}

#[async_trait]
impl UsenetAccounts for Accounts {
    async fn accounts(&self) -> Result<Vec<UsenetAccount>, Failure> {
        match &self.0 {
            Ok(accounts) => Ok(accounts.clone()),
            Err(_) => Err(Failure::Unavailable {
                service: "sabnzbd".to_owned(),
            }),
        }
    }
}

/// An aggregator answering with the indexers it was given, or as one not answering where it
/// was given a failure.
pub struct Indexed(Result<Vec<IndexerUse>, Failure>);

impl Indexed {
    /// An aggregator answering as `answer` says.
    #[must_use]
    pub const fn answering(answer: Result<Vec<IndexerUse>, Failure>) -> Self {
        Self(answer)
    }
}

#[async_trait]
impl Indexers for Indexed {
    async fn indexers(&self, _now: SystemTime) -> Result<Vec<IndexerUse>, Failure> {
        match &self.0 {
            Ok(indexers) => Ok(indexers.clone()),
            Err(_) => Err(Failure::Unavailable {
                service: "prowlarr".to_owned(),
            }),
        }
    }
}
