//! `archive.extract` across its contract: one script run against an extractor in process
//! and through the contract, which must answer alike and be told alike.

use lemonfiber_ports::service::{Curated, Extracting};

use super::{crosses_alike, extract};

/// Every operation of the capability, each answer written down.
async fn script<E: Extracting>(extractor: &E) -> Vec<String> {
    let curators = ["tv", "movies", "music"].map(|media_type| Curated {
        media_type: media_type.to_owned(),
        address: format!("http://{media_type}:80"),
        key: format!("{media_type}-key"),
        folder: format!("/data/downloads/{media_type}"),
    });
    vec![
        format!("{:?}", extractor.watch(&curators).await),
        format!("{:?}", extractor.working().await),
    ]
}

#[tokio::test]
async fn an_extractor_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(script, extract::Adapter, ["watch", "working"]);
}
