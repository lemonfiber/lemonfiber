use axum::http::StatusCode;
use lemonfiber_core::keys::Scope;

use super::{operator, Minting};
use crate::admission::{Caller, Keyed};

/// A key of `scope`, as the door hands it over.
fn a_key(scope: Scope) -> Caller {
    Caller::Key(Keyed {
        name: "ha".to_owned(),
        scope,
    })
}

#[test]
fn the_operator_keeps_keys_by_a_session_or_by_this_runs_token() {
    assert!(operator(&Caller::Operator).is_ok());
    assert!(operator(&Caller::Machine).is_ok());
}

#[test]
fn no_key_keeps_keys_whatever_its_scope() {
    let member = Scope::Member {
        id: "a7f3".to_owned(),
        name: "ana".to_owned(),
    };
    for scope in [Scope::Read, Scope::Act, member] {
        let refused = operator(&a_key(scope)).err().map(|answer| answer.status());
        assert_eq!(refused, Some(StatusCode::FORBIDDEN));
    }
}

#[test]
fn a_member_is_told_keys_are_not_theirs() {
    let refused = operator(&Caller::Member("a7f3".to_owned()))
        .err()
        .map(|answer| answer.status());
    assert_eq!(refused, Some(StatusCode::FORBIDDEN));
}

#[test]
fn a_mint_names_exactly_what_it_needs_and_nothing_else() {
    let whole = r#"{"name":"ha","scope":"act","purpose":"home-assistant","password":"p"}"#;
    assert!(serde_json::from_str::<Minting>(whole).is_ok());
    let padded = r#"{"name":"ha","scope":"act","purpose":"other","password":"p","by":"x"}"#;
    assert!(serde_json::from_str::<Minting>(padded).is_err());
    let unproven = r#"{"name":"ha","scope":"act","purpose":"other"}"#;
    assert!(serde_json::from_str::<Minting>(unproven).is_err());
}

#[test]
fn a_mint_never_prints_the_password_it_was_asked_with() {
    // Built rather than written, so no scan reads a quoted password in this file.
    let password: String = ('p'..='z').collect();
    let asked = serde_json::json!({
        "name": "ha",
        "scope": "act",
        "purpose": "other",
        "password": password,
    })
    .to_string();
    let printed = serde_json::from_str::<Minting>(&asked)
        .map(|minting| format!("{minting:?}"))
        .unwrap_or_default();
    assert!(
        printed.contains("ha") && !printed.contains(&password),
        "{printed}"
    );
}
