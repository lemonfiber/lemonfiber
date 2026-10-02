//! The refusals the contract lists: what a client generates a value for each of.

use std::collections::BTreeSet;

use lemonfiber_api::contract::Contract;
use lemonfiber_core::agreement::MOVED;

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
