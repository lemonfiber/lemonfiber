//! `identity.source` across its contract: one script run against an identity source in
//! process and through the contract, which must answer alike and be told alike.

use lemonfiber_ports::service::{Allowed, Household, MediaServer, Signed, Unrated};

use super::{crosses_alike, source};

/// Every operation of the capability, once, each answer written down.
async fn script<I: MediaServer + Household>(source: &I) -> Vec<String> {
    let allowed = Allowed {
        libraries: Some(vec!["films".to_owned()]),
        age_limit: Some(12),
        unrated: Some(Unrated::HeldBack),
    };
    let chosen: String = ('c'..='n').collect();
    let signed = Signed {
        id: "ana".to_owned(),
        token: "token".to_owned(),
    };
    vec![
        format!("{:?}", source.startup_completed().await),
        format!("{:?}", source.create_admin("admin", "minted").await),
        format!("{:?}", source.household().await),
        format!("{:?}", source.whoever("ana", "right", "phone").await),
        format!("{:?}", source.whoever("ana", "wrong", "phone").await),
        format!("{:?}", source.standing(&signed).await),
        format!("{:?}", source.invite("Sam").await),
        format!("{:?}", source.claim("Sam", &chosen, "phone").await),
        format!("{:?}", source.claim("Ana", &chosen, "phone").await),
        format!("{:?}", source.unclaim("b8e4").await),
        format!("{:?}", source.withdraw("b8e4").await),
        format!("{:?}", source.when_invited("2026-10-01").await),
        format!("{:?}", source.libraries().await),
        format!("{:?}", source.ratings().await),
        format!("{:?}", source.country().await),
        format!("{:?}", source.allow("a7f3", &allowed).await),
        format!("{:?}", source.claimable("b8e4", &Allowed::default()).await),
        format!("{:?}", source.suspend("admin").await),
        format!("{:?}", source.sessions("a7f3").await),
        format!("{:?}", source.signs_devices_in().await),
    ]
}

#[tokio::test]
async fn an_identity_source_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        source::Adapter,
        [
            "admin",
            "claim",
            "claim",
            "unclaim",
            "withdraw",
            "allow",
            "claimable",
            "suspend"
        ]
    );
}
