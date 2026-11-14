//! `stream.guard` across its contract: one script run against a guard in process and
//! through the contract, which must answer alike and be told alike.

use lemonfiber_fixtures::upstream::stream::guarded;
use lemonfiber_ports::service::Guarding;

use super::{crosses_alike, guard};

/// Every operation of the capability, each answer written down.
async fn script<G: Guarding>(guard: &G) -> Vec<String> {
    vec![
        format!("{:?}", guard.guard(&guarded()).await),
        format!("{:?}", guard.guarding().await),
        format!("{:?}", guard.certificate().await),
    ]
}

#[tokio::test]
async fn a_guard_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(script, guard::Adapter, ["guard", "guarding", "certificate"]);
}

#[test]
fn the_declaration_a_guard_is_asked_to_enforce_is_one_a_guard_can() {
    let asked = guard::guard::asks().guarded.declared;
    assert_eq!(asked.clone().checked(), Ok(asked));
}
