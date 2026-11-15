//! `indexer.search`, `indexer.pull` and `indexer.proxy` across their contracts: one script
//! run against each in process and through the contract, which must answer alike and be
//! told alike.

use std::time::{Duration, SystemTime};

use lemonfiber_ports::service::{
    Aggregator, AggregatorAt, Aggregators, AppSync, Application, ApplicationKind, Indexers,
    Protocol, PullsIndexers, RegisteredApplication, Solving,
};

use super::{crosses_alike, pull, search, solver};

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
    crosses_alike!(
        script,
        search::Adapter,
        ["register", "test", "rekey", "aggregator"]
    );
}

/// Every operation of `indexer.pull`, once, each answer written down.
async fn pulled<P: PullsIndexers>(curator: &P) -> Vec<String> {
    let aggregator = AggregatorAt {
        name: "aggregator".to_owned(),
        url: "http://aggregator:9696".to_owned(),
        protocol: Protocol("servarr".to_owned()),
        key: "aggregator-key".to_owned(),
    };
    vec![
        format!("{:?}", PullsIndexers::aggregators(curator).await),
        format!(
            "{:?}",
            PullsIndexers::add_aggregator(curator, &aggregator).await
        ),
        format!("{:?}", PullsIndexers::pull(curator).await),
    ]
}

#[tokio::test]
async fn a_curator_that_pulls_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(pulled, pull::Adapter, ["pull", "again"]);
}

/// Every operation of `indexer.proxy`, once, each answer written down.
async fn solved<S: Solving>(solver: &S) -> Vec<String> {
    vec![
        format!("{:?}", solver.solve("https://indexer.example/").await),
        format!("{:?}", solver.answers().await),
    ]
}

#[tokio::test]
async fn a_challenge_solver_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(solved, solver::Adapter, ["solve", "answers"]);
}
