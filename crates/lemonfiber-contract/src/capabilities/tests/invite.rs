//! `invite.decline` across its contract: one script run against a decline service in
//! process and through the contract, which must answer alike and be told alike.

use lemonfiber_ports::service::{Claimed, Declinable, Declining, Invitations};

use super::{crosses_alike, decline};

/// Every operation of the capability, each answer written down.
async fn script<D: Declining>(decline: &D) -> Vec<String> {
    let invitations = Invitations {
        claimed: Claimed::HasPassword,
        invitations: vec![
            Declinable {
                token: "ab".repeat(32),
                account: "a7f3".to_owned(),
                name: "Ana".to_owned(),
                issued: 1_799_000_000,
                lapses: 1_800_000_000,
            },
            Declinable {
                token: "cd".repeat(32),
                account: "b8e4".to_owned(),
                name: "Bo".to_owned(),
                issued: 1_799_086_400,
                lapses: 1_800_086_400,
            },
        ],
    };
    vec![
        format!(
            "{:?}",
            decline.hold("decline-key", &["ab".repeat(32)]).await
        ),
        format!("{:?}", decline.held().await),
        format!("{:?}", decline.apply_invitations(&invitations).await),
        format!("{:?}", decline.refusals().await),
        format!("{:?}", decline.lapses().await),
        format!("{:?}", decline.page().await),
    ]
}

#[tokio::test]
async fn a_decline_service_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        decline::Adapter,
        [
            "hold",
            "held",
            "apply_invitations",
            "refusals",
            "lapses",
            "page"
        ]
    );
}
