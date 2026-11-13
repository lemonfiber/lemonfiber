//! `status.watch` across its contract: one script run against a watcher in process and
//! through the contract, which must answer alike and be told alike.

use lemonfiber_ports::service::{Covers, Failure, MonitorKind, Monitoring, Provision};

use super::{crosses_alike, watch};

/// A result as the script writes it down: a failure by its kind and words alone, since the
/// service it names is the one asked rather than the one that answered.
fn said<T: std::fmt::Debug>(result: Result<T, Failure>) -> String {
    match result {
        Ok(answer) => format!("{answer:?}"),
        Err(Failure::Refused { detail, .. }) => format!("refused: {detail}"),
        Err(Failure::Disallowed { detail, .. }) => format!("disallowed: {detail}"),
        Err(failure) => format!("{failure}"),
    }
}

/// Every operation of the capability, each answer written down.
async fn script<M: Monitoring>(watcher: &M) -> Vec<String> {
    let provision = |kind| Provision {
        tag: Covers::EntryPoint("jellyfin".to_owned()).tag(),
        kind,
        target: "https://watch.house.lan".to_owned(),
        every_s: 60,
        grace_s: Some(30),
        retries: 3,
    };
    vec![
        said(watcher.reachable().await),
        said(watcher.monitors().await),
        said(watcher.provision(&provision(MonitorKind::Http)).await),
        said(watcher.provision(&provision(MonitorKind::Keyword)).await),
        said(watcher.check_in("lemonfiber:job:guard").await),
        said(watcher.check_in("lemonfiber:service:sonarr").await),
        said(watcher.retire("lemonfiber:service:sonarr").await),
        said(watcher.retire("router").await),
    ]
}

#[tokio::test]
async fn a_watcher_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        watch::Adapter,
        [
            "reachable",
            "monitors",
            "provision",
            "provision",
            "check_in",
            "check_in",
            "retire",
            "retire"
        ]
    );
}
