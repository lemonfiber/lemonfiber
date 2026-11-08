//! A capability whose contract this build speaks and that no bundled service declares yet.

use super::super::awaited;
use super::{against, says, CLAIMANT};
use crate::Manifest;

/// The claimant with its claim and contributions gone, providing the outside watcher's
/// capability beside its own, its service speaking `speaks`.
fn watching(speaks: &str) -> String {
    let (before, rest) = CLAIMANT.split_once("[[claim]]").unwrap_or_default();
    let (_, after) = rest.split_once("[requires]").unwrap_or_default();
    format!("{before}[requires]{after}")
        .replace(
            "provides    = [\"media.serve\", \"komga:opds\"]",
            "provides    = [\"status.watch\", \"komga:opds\"]",
        )
        .replace(
            "criticality = \"enhancing\"\n",
            &format!("criticality = \"enhancing\"\nspeaks      = [{speaks}]\n"),
        )
}

#[test]
fn a_service_speaking_an_awaited_contract_may_provide_it_and_no_other_may() {
    let speaking = against(&watching("\"status.watch@1\""));
    assert!(!says(&speaking, &["status.watch"]), "{speaking:?}");

    let silent = against(&watching("\"identity.source@1\""));
    assert!(
        says(
            &silent,
            &[
                "service komga.provides",
                "status.watch awaits its bundled declarer",
                "speaking its contract"
            ]
        ),
        "{silent:?}"
    );
    assert!(!says(&silent, &["published vocabulary"]), "{silent:?}");
}

#[test]
fn every_awaited_capability_a_manifest_provides_is_named_for_the_caller_to_refuse() {
    let read = Manifest::from_toml(&watching("\"status.watch@1\""));
    assert!(read.is_ok(), "{read:?}");
    let named: Vec<String> = read
        .map(|manifest| awaited(&manifest))
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        named,
        [
            "service komga.provides: status.watch awaits its bundled declarer, and until one is \
          bundled only a first-party plugin may provide it"
        ]
    );

    let claimant = Manifest::from_toml(CLAIMANT);
    assert!(claimant.is_ok_and(|manifest| awaited(&manifest).is_empty()));
}
