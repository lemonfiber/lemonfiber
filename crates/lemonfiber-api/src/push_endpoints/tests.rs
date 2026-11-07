use axum::http::StatusCode;
use lemonfiber_core::keys::Scope;

use super::{operator, Registering};
use crate::admission::{Caller, Keyed};

#[test]
fn only_the_operator_keeps_push_endpoints() {
    assert!(operator(&Caller::Operator).is_ok());
    assert!(operator(&Caller::Machine).is_ok());
    let member = operator(&Caller::Member("a7f3".to_owned()));
    assert_eq!(
        member.err().map(|answer| answer.status()),
        Some(StatusCode::FORBIDDEN)
    );
    for scope in [Scope::Read, Scope::Act] {
        let key = Caller::Key(Keyed {
            name: "ha".to_owned(),
            scope,
        });
        assert_eq!(
            operator(&key).err().map(|answer| answer.status()),
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
