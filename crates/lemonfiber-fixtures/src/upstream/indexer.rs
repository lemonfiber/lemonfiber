//! The upstream as an indexer, a curator that pulls indexers, and a challenge solver.

use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Aggregator, AggregatorAt, Aggregators, AppSync, Application, Cookie, Failure, IndexerUse,
    Indexers, KnownAggregator, Limits, PullsIndexers, RegisteredApplication, Solved, Solving,
};

use super::{Upstream, SERVICE};

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

#[async_trait]
impl PullsIndexers for Upstream {
    async fn aggregators(&self) -> Result<Vec<KnownAggregator>, Failure> {
        Ok(vec![KnownAggregator {
            id: "2".to_owned(),
            url: "http://aggregator:9696".to_owned(),
            keyed: true,
        }])
    }
    async fn add_aggregator(&self, aggregator: &AggregatorAt) -> Result<(), Failure> {
        self.tell(format!("pull from {aggregator:?}"));
        Ok(())
    }
    async fn pull(&self) -> Result<(), Failure> {
        self.tell("again".to_owned());
        Ok(())
    }
}

#[async_trait]
impl Solving for Upstream {
    async fn solve(&self, url: &str) -> Result<Solved, Failure> {
        self.tell(format!("solve {url}"));
        Ok(Solved {
            status: 200,
            url: url.to_owned(),
            user_agent: "Mozilla/5.0".to_owned(),
            cookies: vec![Cookie {
                name: "clearance".to_owned(),
                value: "cleared".to_owned(),
                domain: "indexer.example".to_owned(),
            }],
            page: "<html></html>".to_owned(),
        })
    }
    async fn answers(&self) -> Result<bool, Failure> {
        self.tell("answers".to_owned());
        Ok(true)
    }
}
