//! `owner.claim` across its contract: one script run against a part in process and through
//! the contract, which must answer alike and be told alike.

use lemonfiber_ports::service::Ownership;

use super::{claim, crosses_alike};

/// Every operation of the capability, each answer written down.
async fn script<O: Ownership>(part: &O) -> Vec<String> {
    vec![
        format!("{:?}", part.owner().await),
        format!("{:?}", part.claim().await),
        format!("{:?}", part.claimed_by().await),
    ]
}

#[tokio::test]
async fn a_part_claimed_first_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(script, claim::Adapter, ["owner", "claim", "claimed-by"]);
}
