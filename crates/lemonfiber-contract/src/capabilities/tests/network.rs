//! `network.egress-guard` across its contract: one script run against an egress guard in
//! process and through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Tunnelling};

use super::{crosses_alike, egress, Upstream};

#[async_trait]
impl Tunnelling for Upstream {
    async fn up(&self) -> Result<bool, Failure> {
        self.tell("up".to_owned());
        Ok(true)
    }
    async fn forwarded(&self) -> Result<Option<u16>, Failure> {
        self.tell("forwarded".to_owned());
        Ok(Some(51413))
    }
}

/// Every operation of the capability, each answer written down.
async fn script<T: Tunnelling>(guard: &T) -> Vec<String> {
    vec![
        format!("{:?}", guard.up().await),
        format!("{:?}", guard.forwarded().await),
    ]
}

#[tokio::test]
async fn an_egress_guard_answers_through_its_contract_as_it_does_in_process() {
    crosses_alike!(script, egress::Adapter, ["up", "forwarded"]);
}
