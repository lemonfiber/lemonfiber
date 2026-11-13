//! `network.egress-guard` across its contract: one script run against an egress guard in
//! process and through the contract, which must answer alike and be told alike.

use lemonfiber_fixtures::upstream::network::Unreached;
use lemonfiber_ports::service::Tunnelling;

use super::{crosses_alike, egress};

/// Every operation of the capability, each answer written down.
async fn script<T: Tunnelling>(guard: &T) -> Vec<String> {
    vec![
        format!("{:?}", guard.up().await),
        format!("{:?}", guard.forwarded().await),
        format!("{:?}", guard.secure().await),
    ]
}

#[tokio::test]
async fn an_egress_guard_answers_through_its_contract_as_it_does_in_process() {
    crosses_alike!(script, egress::Adapter, ["up", "forwarded", "secure"]);
}

#[tokio::test]
async fn an_egress_guard_is_served_its_answers_its_failures_and_nothing_else() {
    let refused = |served: Result<crate::wire::Answer, crate::wire::Refusal>| {
        served.err().map(|refusal| refusal.kind)
    };
    assert_eq!(
        refused(egress::dispatch(&Unreached, "up", b"{}").await),
        Some(crate::wire::Kind::Unavailable)
    );
    assert_eq!(
        egress::dispatch(&Unreached, "forwarded", b"{}").await.ok(),
        Some(crate::wire::Answer::Json(b"null".to_vec()))
    );
    assert_eq!(
        egress::dispatch(&Unreached, "secure", b"{}").await.ok(),
        Some(crate::wire::Answer::Reset)
    );
    assert_eq!(
        refused(egress::dispatch(&Unreached, "forwarded", b"not a request").await),
        Some(crate::wire::Kind::NotAsked)
    );
    assert_eq!(
        refused(egress::dispatch(&Unreached, "peer_port", b"{}").await),
        Some(crate::wire::Kind::Unsupported)
    );
}
