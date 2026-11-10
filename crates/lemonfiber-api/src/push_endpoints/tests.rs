use axum::http::StatusCode;
use lemonfiber_core::keys::Scope;

use super::{owner, Registering};
use crate::admission::{Caller, Keyed};

#[test]
fn the_operator_keeps_every_push_endpoint_a_member_their_own_and_a_key_none() {
    assert_eq!(owner(&Caller::Operator).ok(), Some(None));
    assert_eq!(owner(&Caller::Machine).ok(), Some(None));
    assert_eq!(
        owner(&Caller::Member("a7f3".to_owned())).ok(),
        Some(Some("a7f3".to_owned()))
    );
    for scope in [Scope::Read, Scope::Act] {
        let key = Caller::Key(Keyed {
            name: "ha".to_owned(),
            scope,
        });
        assert_eq!(
            owner(&key).err().map(|answer| answer.status()),
            Some(StatusCode::FORBIDDEN)
        );
    }
}

#[test]
fn a_registration_never_prints_its_keys_or_its_address() {
    let registering = Registering {
        name: "pixel".to_owned(),
        endpoint: "https://push.example/the-address".to_owned(),
        p256dh: "the-public-key".to_owned(),
        auth: "the-secret".to_owned(),
    };
    let said = format!("{registering:?}");
    assert!(said.contains("pixel"), "{said}");
    for withheld in ["the-address", "the-public-key", "the-secret"] {
        assert!(!said.contains(withheld), "{said}");
    }
}
