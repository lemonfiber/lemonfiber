use std::collections::BTreeSet;

use axum::body::to_bytes;
use axum::http::StatusCode;
use lemonfiber_core::error::codes::declared;
use lemonfiber_core::model::{kind, Envelope};
use serde_json::Value;

use super::{Refusal, UNRENDERED};
use crate::contract::Contract;
use crate::read::enveloped;

/// The body a response carries, read as JSON.
async fn body_of(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .map(|bytes| bytes.to_vec())
        .unwrap_or_default();
    serde_json::from_slice(&bytes).unwrap_or_default()
}

#[test]
fn every_refusal_carries_a_code_of_its_own() {
    let codes: BTreeSet<&str> = Refusal::EVERY
        .iter()
        .map(|refusal| refusal.code().as_str())
        .collect();
    assert_eq!(codes.len(), Refusal::EVERY.len());
}

#[test]
fn every_code_a_refusal_carries_is_one_the_registry_declares() {
    for refusal in Refusal::EVERY {
        assert!(declared(refusal.code()).is_some(), "{refusal:?}");
    }
}

#[test]
fn every_refusal_says_its_own_sentence_and_what_it_means() {
    let said: BTreeSet<&str> = Refusal::EVERY
        .iter()
        .map(|refusal| refusal.said())
        .collect();
    assert_eq!(said.len(), Refusal::EVERY.len());
    for refusal in Refusal::EVERY {
        let problem = refusal.problem(refusal.said());
        assert!(!problem.meaning.is_empty(), "{refusal:?}");
        assert!(!problem.remedies.is_empty(), "{refusal:?}");
        assert_eq!(problem.code, refusal.code());
    }
}

#[test]
fn a_refusal_raised_as_a_problem_answers_at_its_status() {
    for refusal in Refusal::EVERY {
        assert_eq!(
            refusal.problem("").status(),
            refusal.status().as_u16(),
            "{refusal:?}"
        );
    }
}

#[test]
fn only_the_password_door_answers_unauthorized() {
    let unauthorized: Vec<Refusal> = Refusal::EVERY
        .into_iter()
        .filter(|refusal| refusal.status() == StatusCode::UNAUTHORIZED)
        .collect();
    assert_eq!(unauthorized, vec![Refusal::NotThePassword]);
}

#[test]
fn the_refusals_of_who_is_asking_share_a_status_and_not_a_code() {
    let forbidden: Vec<Refusal> = Refusal::EVERY
        .into_iter()
        .filter(|refusal| refusal.status() == StatusCode::FORBIDDEN)
        .collect();
    assert_eq!(
        forbidden,
        vec![
            Refusal::NotAdmitted,
            Refusal::Elsewhere,
            Refusal::NotYours,
            Refusal::Unconfirmed,
            Refusal::KeyInTheClear,
            Refusal::NotForAKey,
        ]
    );
}

#[test]
fn only_the_refusals_that_are_not_faults_are_warnings() {
    for refusal in Refusal::EVERY {
        let warns = matches!(
            refusal,
            Refusal::NotYours
                | Refusal::Unconfirmed
                | Refusal::TooManyAttempts
                | Refusal::NotForAKey
        );
        let severity = refusal.problem("").severity;
        assert_eq!(
            severity == lemonfiber_core::error::Severity::Warning,
            warns,
            "{refusal:?}"
        );
    }
}

#[tokio::test]
async fn a_refusal_answers_with_the_error_envelope_carrying_its_code() {
    for refusal in Refusal::EVERY {
        let response = refusal.answered();
        assert_eq!(response.status(), refusal.status(), "{refusal:?}");
        let body = body_of(response).await;
        assert_eq!(
            body.pointer("/kind").and_then(serde_json::Value::as_str),
            Some("error"),
            "{refusal:?}"
        );
        assert_eq!(
            body.pointer("/data/code")
                .and_then(serde_json::Value::as_str),
            Some(refusal.code().as_str()),
            "{refusal:?}"
        );
        assert_eq!(
            body.pointer("/data/summary")
                .and_then(serde_json::Value::as_str),
            Some(refusal.said()),
            "{refusal:?}"
        );
    }
}

#[tokio::test]
async fn a_refusal_saying_its_particulars_keeps_its_code() {
    let response = Refusal::NoSuchAction.saying("There is no action named `x`.");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_of(response).await;
    assert_eq!(
        body.pointer("/data/code")
            .and_then(serde_json::Value::as_str),
        Some("ASK-1")
    );
    assert_eq!(
        body.pointer("/data/summary")
            .and_then(serde_json::Value::as_str),
        Some("There is no action named `x`.")
    );
}

#[test]
fn the_refusal_written_out_ahead_is_the_one_rendering_would_write() {
    let problem = Refusal::Unrenderable.problem(Refusal::Unrenderable.said());
    let rendered = Envelope::new(kind::ERROR, problem).to_json();
    assert_eq!(rendered.as_deref(), Some(UNRENDERED));
}

#[tokio::test]
async fn nothing_rendered_is_answered_as_the_refusal_saying_so() {
    let response = enveloped(StatusCode::OK, None);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = body_of(response).await;
    assert_eq!(
        body.pointer("/data/code")
            .and_then(serde_json::Value::as_str),
        Some(Refusal::Unrenderable.code().as_str())
    );
}

#[test]
fn the_contract_lists_every_refusal_at_the_status_it_is_answered_with() {
    let listed = Contract::describe().refusals;
    // This surface's own, the core's refusals of an offer that has moved, which
    // `contract/refusals` holds to their status, and what the plugins and wiring reads
    // are refused with where what they read could not be read, and what a choice of
    // filler and a plugin's install, update or removal are refused with, and a member's
    // viewing.
    let unread: usize = lemonfiber_core::wiring::UNREAD
        .iter()
        .map(|codes| codes.len())
        .sum();
    assert_eq!(
        listed.len(),
        Refusal::EVERY.len()
            + lemonfiber_core::agreement::MOVED.len()
            + unread
            + lemonfiber_core::wiring::REFUSED.len()
            + lemonfiber_core::app::plugins::REFUSALS.len()
            + lemonfiber_core::screening::REFUSALS.len()
    );
    for code in lemonfiber_core::wiring::UNREAD
        .iter()
        .flat_map(|codes| codes.iter())
    {
        assert_eq!(
            listed.get(code.as_str()).map(|entry| entry.status),
            Some(StatusCode::INTERNAL_SERVER_ERROR.as_u16()),
            "{code:?} is listed at the status a failure of the machine is answered with"
        );
    }
    for refusal in Refusal::EVERY {
        let entry = listed.get(refusal.code().as_str());
        assert_eq!(
            entry.map(|entry| entry.status),
            Some(refusal.status().as_u16()),
            "{refusal:?}"
        );
    }
}
