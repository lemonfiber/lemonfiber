use axum::http::{HeaderMap, HeaderValue, StatusCode};
use lemonfiber_core::keys::Scope;

use super::{asking, operator, Registering};
use crate::admission::{Caller, Keyed};
use crate::guard::TOKEN_HEADER;

#[test]
fn only_the_operator_keeps_device_keys() {
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
fn a_challenge_is_bound_to_the_token_the_request_carried() {
    let mut headers = HeaderMap::new();
    assert_eq!(asking(&headers), None);
    headers.insert(TOKEN_HEADER, HeaderValue::from_static("a-token"));
    assert_eq!(asking(&headers), Some("a-token"));
}

#[test]
fn a_registration_never_prints_the_password_or_the_public_half() {
    let password: String = ('p'..='z').collect();
    let asked = serde_json::json!({
        "name": "pixel",
        "public_key": "the-public-half",
        "attestation": { "platform": "ios", "object": "the-object" },
        "password": password,
    })
    .to_string();
    let printed = serde_json::from_str::<Registering>(&asked)
        .map(|registering| format!("{registering:?}"))
        .unwrap_or_default();
    assert!(
        printed.contains("pixel") && printed.contains("ios"),
        "{printed}"
    );
    for withheld in [password.as_str(), "the-public-half", "the-object"] {
        assert!(!printed.contains(withheld), "{printed}");
    }
}

#[test]
fn a_registration_names_exactly_what_it_needs_and_an_attestation_its_platform() {
    let unknown = serde_json::json!({
        "name": "pixel", "public_key": "k", "password": "p", "by": "x",
    });
    assert!(serde_json::from_value::<Registering>(unknown).is_err());
    let elsewhere = serde_json::json!({
        "name": "pixel", "public_key": "k", "password": "p",
        "attestation": { "platform": "windows", "chain": [] },
    });
    assert!(serde_json::from_value::<Registering>(elsewhere).is_err());
    let unattested = serde_json::json!({ "name": "pixel", "public_key": "k", "password": "p" });
    assert!(serde_json::from_value::<Registering>(unattested)
        .is_ok_and(|registering| registering.attestation.is_none()));
}
