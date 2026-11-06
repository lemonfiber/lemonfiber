//! `owner.claim` across its contract: one script run against a part in process and through
//! the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{Claim, Failure, Owner, Ownership};

use super::{claim, crosses_alike, Upstream};

#[async_trait]
impl Ownership for Upstream {
    async fn owner(&self) -> Result<Owner, Failure> {
        self.tell("owner".to_owned());
        Ok(Owner::Open)
    }
    async fn claim(&self) -> Result<Claim, Failure> {
        self.tell("claim".to_owned());
        Ok(Claim::ON_RESTART)
    }
    async fn claimed_by(&self) -> Result<Option<String>, Failure> {
        self.tell("claimed-by".to_owned());
        Ok(Some("4815162342".to_owned()))
    }
}

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
