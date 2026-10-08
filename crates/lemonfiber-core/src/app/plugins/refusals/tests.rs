use super::REFUSALS;
use crate::error::codes::plugin::{HEADER_NAMED, REFUSED};

/// Each code once, each a plugin's, and none listed beside the reads or the moved
/// offers already: the published list is keyed by code, so a code in two lists would
/// be one entry with one status, whichever came last.
#[test]
fn each_code_is_listed_once_and_nowhere_else() {
    let mut codes: Vec<&str> = REFUSALS.iter().map(|code| code.as_str()).collect();
    assert!(codes.iter().all(|code| code.starts_with("PLUGIN-")));
    let elsewhere: Vec<&str> = crate::agreement::MOVED
        .iter()
        .chain(crate::wiring::UNREAD.iter().flat_map(|codes| codes.iter()))
        .map(|code| code.as_str())
        .collect();
    assert!(
        codes.iter().all(|code| !elsewhere.contains(code)),
        "{codes:?}"
    );
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), REFUSALS.len());
}

/// A manifest refused with a code of its own for one of its faults is still a manifest
/// refused, and is answered as a refused manifest is.
#[test]
fn a_header_named_by_substitution_is_answered_as_a_refused_manifest_is() {
    assert_eq!(HEADER_NAMED.status(), REFUSED.status());
    assert_eq!(HEADER_NAMED.status(), 400);
}
