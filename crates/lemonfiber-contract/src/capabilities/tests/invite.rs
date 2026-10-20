//! `invite.decline` across its contract: one script run against a decline service in
//! process and through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{Declinable, Declining, Failure};

use super::{crosses_alike, decline, Upstream};

#[async_trait]
impl Declining for Upstream {
    async fn hold(&self, key: &str) -> Result<(), Failure> {
        self.tell(format!("hold {key}"));
        Ok(())
    }
    async fn held(&self) -> Result<Option<String>, Failure> {
        self.tell("held".to_owned());
        Ok(Some(lemonfiber_ports::fingerprint("decline-key")))
    }
    async fn apply_invitations(&self, invitations: &[Declinable]) -> Result<(), Failure> {
        self.tell(format!("apply_invitations {invitations:?}"));
        Ok(())
    }
}

/// Every operation of the capability, each answer written down.
async fn script<D: Declining>(decline: &D) -> Vec<String> {
    let invitations = [
        Declinable {
            token: "ab".repeat(32),
            account: "a7f3".to_owned(),
            lapses: 1_800_000_000,
        },
        Declinable {
            token: "cd".repeat(32),
            account: "b8e4".to_owned(),
            lapses: 1_800_086_400,
        },
    ];
    vec![
        format!("{:?}", decline.hold("decline-key").await),
        format!("{:?}", decline.held().await),
        format!("{:?}", decline.apply_invitations(&invitations).await),
    ]
}

#[tokio::test]
async fn a_decline_service_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        decline::Adapter,
        ["hold", "held", "apply_invitations"]
    );
}
