use super::{place, REFUSALS};
use crate::error::codes::plugin::{CONTRIBUTED_FAILED, HEADER_NAMED, NOTHING_TO_REMOVE, REFUSED};
use crate::error::{Amiss, Problem, Remedy, Severity};

/// A problem carrying this code, raised where nothing has placed it.
fn raised(code: crate::error::Code) -> Problem {
    Problem::new(
        code,
        Severity::Error,
        "summary",
        "meaning",
        Remedy::new("remedy"),
    )
}

#[test]
fn a_listed_refusal_is_placed_where_the_list_says() {
    let mut refused = raised(NOTHING_TO_REMOVE);
    place(&mut refused);
    assert_eq!(refused.amiss, Amiss::Naming);
}

#[test]
fn a_refusal_the_list_does_not_hold_keeps_its_place() {
    let mut kept = raised(CONTRIBUTED_FAILED).lies_in(Amiss::Held);
    place(&mut kept);
    assert_eq!(kept.amiss, Amiss::Held);
}

/// Each code once, each a plugin's, and none listed beside the reads or the moved
/// offers already: the published list is keyed by code, so a code in two lists would
/// be one entry with one status, whichever came last.
#[test]
fn each_code_is_listed_once_and_nowhere_else() {
    let mut codes: Vec<&str> = REFUSALS.iter().map(|(code, _)| code.as_str()).collect();
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
/// refused, and lies where the manifest's refusal does.
#[test]
fn a_header_named_by_substitution_lies_where_a_refused_manifest_does() {
    let mut named = raised(HEADER_NAMED);
    let mut refused = raised(REFUSED);
    place(&mut named);
    place(&mut refused);
    assert_eq!(named.amiss, refused.amiss);
    assert_eq!(named.amiss, Amiss::Asking);
}
