//! `indexer.search` across its contract: one script run against an indexer in process and
//! through the contract, which must answer alike and be told alike.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Aggregator, Aggregators, AppSync, Application, ApplicationKind, Failure, IndexerUse, Indexers,
    KnownAggregator, Limits, RegisteredApplication,
};

use super::{contracted, search, Served, Upstream, SERVICE};

#[async_trait]
impl AppSync for Upstream {
    async fn register_application(&self, application: &Application) -> Result<(), Failure> {
        self.tell(format!("register {application:?}"));
        Ok(())
    }
    async fn applications(&self) -> Result<Vec<RegisteredApplication>, Failure> {
        Ok(vec![RegisteredApplication {
            id: "4".to_owned(),
            base_url: "http://films:7878".to_owned(),
        }])
    }
    async fn test_application(&self, held: &RegisteredApplication) -> Result<(), Failure> {
        self.tell(format!("test {held:?}"));
        Err(Failure::Refused {
            service: SERVICE.to_owned(),
            detail: "it does not answer".to_owned(),
        })
    }
    async fn rekey_application(
        &self,
        held: &RegisteredApplication,
        key: &str,
    ) -> Result<(), Failure> {
        self.tell(format!("rekey {held:?} {key}"));
        Ok(())
    }
}

#[async_trait]
impl Indexers for Upstream {
    async fn indexers(&self, now: SystemTime) -> Result<Vec<IndexerUse>, Failure> {
        Ok(vec![IndexerUse {
            name: "a source".to_owned(),
            enabled: true,
            queries: 40,
            failed_queries: 1,
            grabs: 3,
            failed_grabs: 0,
            rested_until: None,
            limits: Some(Limits {
                queries: Some(100),
                grabs: None,
                window: Duration::from_secs(86_400),
            }),
            searched_from: Some(now),
            grabbed_from: None,
        }])
    }
}

#[async_trait]
impl Aggregators for Upstream {
    async fn aggregators(&self) -> Result<Vec<KnownAggregator>, Failure> {
        Ok(vec![KnownAggregator {
            id: "1".to_owned(),
            url: "http://aggregator:5076".to_owned(),
            keyed: false,
        }])
    }
    async fn add_aggregator(&self, aggregator: &Aggregator) -> Result<(), Failure> {
        self.tell(format!("aggregator {aggregator:?}"));
        Ok(())
    }
}

/// Every operation of the capability, once, each answer written down.
async fn script<I: AppSync + Indexers + Aggregators>(indexer: &I) -> Vec<String> {
    let application = Application {
        name: "films".to_owned(),
        kind: ApplicationKind::Movies,
        indexer_url: "http://indexer:9696".to_owned(),
        base_url: "http://films:7878".to_owned(),
        api_key: "films-key".to_owned(),
    };
    let held = RegisteredApplication {
        id: "4".to_owned(),
        base_url: "http://films:7878".to_owned(),
    };
    let aggregator = Aggregator {
        name: "aggregator".to_owned(),
        url: "http://aggregator:5076".to_owned(),
        key: "aggregator-key".to_owned(),
    };
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_500_000);
    vec![
        format!("{:?}", indexer.register_application(&application).await),
        format!("{:?}", indexer.applications().await),
        format!("{:?}", indexer.test_application(&held).await),
        format!("{:?}", indexer.rekey_application(&held, "new-key").await),
        format!("{:?}", indexer.indexers(now).await),
        format!("{:?}", indexer.aggregators().await),
        format!("{:?}", indexer.add_aggregator(&aggregator).await),
    ]
}

#[tokio::test]
async fn an_indexer_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    let in_process = Upstream::default();
    let local = script(&in_process).await;
    let served = Served::default();
    let reached = Arc::clone(&served.upstream);
    let crossed = script(&search::Adapter(contracted(served))).await;
    assert_eq!(crossed, local);
    assert_eq!(reached.told(), in_process.told());
    assert_eq!(in_process.told().len(), 4);
}
