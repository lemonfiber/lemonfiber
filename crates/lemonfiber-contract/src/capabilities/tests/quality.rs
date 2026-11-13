//! `quality.sync` across its contract: one script run against a quality sync in process
//! and through the contract, which must answer alike and be told alike.

use lemonfiber_ports::media::preset::Preset;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Profiled, Syncing};

use super::{crosses_alike, sync};

/// Every operation of the capability, each answer written down.
async fn script<S: Syncing>(sync: &S) -> Vec<String> {
    let profiled = Profiled {
        which: Kind::Movies,
        host: "films".to_owned(),
        port: 7878,
        api_key: "films-key".to_owned(),
        preset: Preset::Maximum,
    };
    vec![
        format!("{:?}", sync.apply(&profiled).await),
        format!("{:?}", sync.applied(Kind::Movies).await),
        format!("{:?}", sync.applied(Kind::Tv).await),
        format!("{:?}", sync.edited(&profiled).await),
        format!("{:?}", sync.reapply(&profiled).await),
    ]
}

#[tokio::test]
async fn a_quality_sync_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        sync::Adapter,
        ["apply", "applied", "applied", "edited", "reapply"]
    );
}
