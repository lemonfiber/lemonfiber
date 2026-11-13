use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{Released, RELEASED};
use crate::documents::DIRECTORY;

/// One capability, each major given with the operations it holds.
fn holding(capability: &str, majors: &[(u32, &[&str])]) -> Released {
    let majors: BTreeMap<u32, BTreeSet<String>> = majors
        .iter()
        .map(|(major, operations)| {
            (
                *major,
                operations
                    .iter()
                    .map(|operation| (*operation).to_owned())
                    .collect(),
            )
        })
        .collect();
    Released {
        capabilities: BTreeMap::from([(capability.to_owned(), majors)]),
    }
}

/// The index the last release committed.
fn committed() -> Released {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(DIRECTORY)
        .join(RELEASED);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[test]
fn the_index_this_build_publishes_drops_nothing_the_last_release_published() {
    let committed = committed();
    assert!(
        !committed.capabilities.is_empty(),
        "{RELEASED} is committed"
    );
    assert_eq!(
        committed.dropped_by(&Released::published()),
        Vec::<String>::new()
    );
}

#[test]
fn an_operation_dropped_under_an_unchanged_major_is_named() {
    let before = holding("library.curate", &[(1, &["identity", "queue"])]);
    let after = holding("library.curate", &[(1, &["identity"])]);
    assert_eq!(
        before.dropped_by(&after),
        vec!["library.curate v1 drops queue without a new major".to_owned()]
    );
}

#[test]
fn an_operation_added_drops_nothing() {
    let before = holding("library.curate", &[(1, &["identity"])]);
    let after = holding("library.curate", &[(1, &["identity", "queue"])]);
    assert!(before.dropped_by(&after).is_empty());
}

#[test]
fn a_major_retired_beside_a_higher_one_drops_nothing() {
    let before = holding("library.curate", &[(1, &["identity", "queue"])]);
    let after = holding("library.curate", &[(2, &["identity"])]);
    assert!(before.dropped_by(&after).is_empty());
}

#[test]
fn a_major_retired_with_no_higher_one_is_named() {
    let before = holding("library.curate", &[(2, &["identity"])]);
    for after in [
        holding("library.curate", &[(1, &["identity"])]),
        Released::default(),
    ] {
        assert_eq!(
            before.dropped_by(&after),
            vec![
                "library.curate v2 is no longer listed, and no higher major of library.curate is"
                    .to_owned()
            ]
        );
    }
}

#[test]
fn every_major_a_capability_declares_is_listed_with_its_operations() {
    let capabilities = crate::capabilities::all();
    let published = Released::published();
    for capability in &capabilities {
        let held = published
            .capabilities
            .get(capability.name)
            .and_then(|majors| majors.get(&capability.major));
        assert_eq!(
            held.map(BTreeSet::len),
            Some(capability.operations.len()),
            "{}",
            capability.name
        );
    }
}
