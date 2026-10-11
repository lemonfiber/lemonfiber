//! The refusals the contract lists: what a client generates a value for each of.

use std::collections::BTreeSet;

use axum::http::StatusCode;
use lemonfiber_api::contract::Contract;
use lemonfiber_core::agreement::MOVED;
use lemonfiber_core::error::codes::{self, plugin, quota};
use lemonfiber_core::error::Code;

/// A generator names a value per refusal after the name it is declared under, so two
/// codes declared under one name would be one value standing for two refusals — and
/// both SDKs refuse such an artefact rather than choose.
#[test]
fn every_refusal_listed_goes_by_a_name_of_its_own() {
    let listed = Contract::describe().refusals;
    let names: BTreeSet<&str> = listed.values().map(|refusal| refusal.name).collect();
    assert_eq!(names.len(), listed.len(), "{listed:?}");
}

/// An answer refused for naming what has since moved is the one refusal a client
/// answers by reading again, so each code it carries is listed, at the status that
/// says the request could not be answered as it was asked — never at a failure's.
#[test]
fn every_code_a_moved_offer_ends_with_is_listed_as_an_answer_to_correct() {
    let listed = Contract::describe().refusals;
    for code in MOVED {
        let status = listed.get(code.as_str()).map(|refusal| refusal.status);
        assert_eq!(status, Some(400), "{code:?}");
    }
}

/// A claim the collection door turns away is named at the status that door answers
/// with, which a client tells apart from a refused key by it.
#[test]
fn a_claim_that_collects_nothing_is_listed_as_unauthorised() {
    let listed = Contract::describe().refusals;
    let claim = listed
        .get(codes::key::CLAIM_NOT_OPEN.as_str())
        .map(|refusal| refusal.status);
    assert_eq!(claim, Some(StatusCode::UNAUTHORIZED.as_u16()));
}

/// The codes at a status of the client's own making that no request on this surface is
/// answered with: an arrangement for closing requests is made at a shell, and a check a
/// plugin contributed fails inside a diagnosis rather than refusing one.
const ANSWERED_ELSEWHERE: [Code; 3] = [
    quota::NOTHING_AGREED,
    quota::TOO_SOON,
    plugin::CONTRIBUTED_FAILED,
];

/// A code at a status of the client's own making is one a client branches on, so each
/// one a request on this surface can be answered with is listed, at that status.
#[test]
fn every_code_a_client_is_answered_for_its_own_request_with_is_listed() {
    let listed = Contract::describe().refusals;
    for declared in codes::every_declared() {
        let code = declared.code();
        let theirs =
            StatusCode::from_u16(declared.status()).is_ok_and(|status| status.is_client_error());
        if !theirs || ANSWERED_ELSEWHERE.contains(&code) {
            continue;
        }
        let status = listed.get(code.as_str()).map(|refusal| refusal.status);
        assert_eq!(status, Some(declared.status()), "{code:?}");
    }
    for code in ANSWERED_ELSEWHERE {
        assert!(!listed.contains_key(code.as_str()), "{code:?}");
    }
}
