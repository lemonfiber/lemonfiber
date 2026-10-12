use axum::http::StatusCode;
use lemonfiber_core::keys::Scope;

use lemonfiber_core::keys::Minter;

use super::{keeper, proof, Collecting, Minting, Proof};
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
    assert_eq!(keeper(&Caller::Operator).ok(), Some(Minter::Operator));
    assert_eq!(keeper(&Caller::Machine).ok(), Some(Minter::Operator));
}

#[test]
fn no_key_keeps_keys_whatever_its_scope() {
    let member = Scope::Member {
        id: "a7f3".to_owned(),
        name: "ana".to_owned(),
    };
    for scope in [Scope::Read, Scope::Act, member] {
        let refused = keeper(&a_key(scope)).err().map(|answer| answer.status());
        assert_eq!(refused, Some(StatusCode::FORBIDDEN));
    }
}

#[test]
fn a_member_keeps_keys_as_themselves() {
    let kept = keeper(&Caller::Member("a7f3".to_owned())).ok();
    assert_eq!(
        kept,
        Some(Minter::Member {
            id: "a7f3".to_owned()
        })
    );
}

#[test]
fn a_mint_names_exactly_what_it_needs_and_nothing_else() {
    let whole = r#"{"name":"ha","scope":"act","purpose":"home-assistant","password":"p"}"#;
    assert!(serde_json::from_str::<Minting>(whole).is_ok());
    let padded = r#"{"name":"ha","scope":"act","purpose":"other","password":"p","by":"x"}"#;
    assert!(serde_json::from_str::<Minting>(padded).is_err());
    let unproven = r#"{"name":"ha","scope":"act","purpose":"other"}"#;
    let unproven = serde_json::from_str::<Minting>(unproven);
    assert!(unproven.is_ok_and(|minting| proof(&minting).is_none()));
}

#[test]
fn a_mint_carries_the_password_or_the_whole_of_a_device_keys_proof_and_never_both() {
    let mint = |extra: serde_json::Value| {
        let mut body = serde_json::json!({"name": "ha", "scope": "act", "purpose": "other"});
        if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
            body.extend(extra.clone());
        }
        serde_json::from_value::<Minting>(body)
            .ok()
            .and_then(|minting| proof(&minting))
    };
    let device = serde_json::json!({"device": "pixel", "challenge": "c", "signature": "s"});
    assert!(matches!(mint(device.clone()), Some(Proof::Device(_))));
    assert!(matches!(
        mint(serde_json::json!({"password": "p"})),
        Some(Proof::Password(_))
    ));
    let mut both = device;
    if let Some(both) = both.as_object_mut() {
        both.insert("password".to_owned(), serde_json::json!("p"));
    }
    assert!(mint(both).is_none());
    assert!(mint(serde_json::json!({"device": "pixel", "challenge": "c"})).is_none());
}

#[test]
fn a_mint_never_prints_the_challenge_or_the_signature_it_was_proved_with() {
    let asked = serde_json::json!({
        "name": "ha", "scope": "act", "purpose": "other",
        "device": "pixel", "challenge": "the-challenge", "signature": "the-signature",
    })
    .to_string();
    let printed = serde_json::from_str::<Minting>(&asked)
        .map(|minting| format!("{minting:?}"))
        .unwrap_or_default();
    assert!(printed.contains("pixel"), "{printed}");
    assert!(!printed.contains("the-challenge") && !printed.contains("the-signature"));
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

#[test]
fn a_collection_takes_the_claim_alone_and_never_prints_it() {
    let claim = format!("lfc_{}", "7".repeat(64));
    let asked = serde_json::json!({ "claim": claim }).to_string();
    let printed = serde_json::from_str::<Collecting>(&asked)
        .map(|collecting| format!("{collecting:?}"))
        .unwrap_or_default();
    assert!(
        printed.contains("withheld") && !printed.contains(&claim),
        "{printed}"
    );
    let padded = serde_json::json!({ "claim": claim, "name": "ha" }).to_string();
    assert!(serde_json::from_str::<Collecting>(&padded).is_err());
}
