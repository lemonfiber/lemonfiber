use std::collections::BTreeSet;

use super::{categories, published, EXTENSION_POINTS_VERSION, POINTS};

#[test]
fn it_publishes_the_two_points_a_plugin_may_contribute_at() {
    let named: Vec<&str> = POINTS.iter().map(|point| point.name).collect();
    assert_eq!(named, vec!["doctor.check", "doctor.remedy"]);
}

/// The two a rule elsewhere is about are the two this publishes, by identity rather
/// than by a name written down twice.
#[test]
fn the_two_points_a_rule_names_are_the_two_that_are_published() {
    assert!(POINTS.iter().any(|point| point.name == super::check()));
    assert!(POINTS.iter().any(|point| point.name == super::remedy()));
    assert_ne!(super::check(), super::remedy());
}

#[test]
fn a_check_is_bounded_here_rather_than_by_the_plugin_s_opinion() {
    let bounds = POINTS
        .iter()
        .find(|point| point.name == "doctor.check")
        .and_then(|point| point.row.bounds.first())
        .map(|bound| {
            (
                bound.field,
                bound.limits.min,
                bound.limits.max,
                bound.limits.default,
            )
        });
    assert_eq!(bounds, Some(("timeout_s", 1, 30, 10)));
}

/// The bound the register is held to is the bound the artefact publishes.
///
/// One value under two names would be two numbers to keep in step, and the one
/// that fell behind would be the one actually bounding a run — leaving the
/// published document describing a limit nothing applies.
#[test]
fn the_published_bound_and_the_one_a_runner_reads_are_the_same_value() {
    let published = POINTS
        .iter()
        .find(|point| point.name == super::check())
        .and_then(|point| point.row.bounds.first())
        .map(|bound| bound.limits);
    assert_eq!(published, Some(super::CHECK_TIMEOUT));
}

#[test]
fn a_check_is_narrowed_to_one_of_the_nine_families_the_doctor_recognises() {
    let values = POINTS
        .iter()
        .find(|point| point.name == "doctor.check")
        .and_then(|point| point.row.enums.first())
        .map(|closed| (closed.field, closed.values.len()));
    assert_eq!(values, Some(("category", 9)));
    assert_eq!(categories().len(), 9);
}

/// A check asks the plugin's own service and nothing else.
///
/// There is no field for a host, so a check cannot be pointed at another service,
/// at the machine, or off it — and the way to keep that true is for the row to
/// name no such field rather than for something downstream to refuse one.
#[test]
fn a_contributed_check_has_no_field_by_which_it_could_name_another_host() {
    let reaching: Vec<&str> = POINTS
        .iter()
        .flat_map(|point| point.row.required.iter().chain(point.row.optional))
        .copied()
        .filter(|field| matches!(*field, "host" | "url" | "to" | "address" | "machine"))
        .collect();
    assert!(reaching.is_empty(), "a row could name one: {reaching:?}");
}

#[test]
fn the_identities_the_doctor_holds_are_published_against_the_check_point() {
    let artefact = published(&["storage.space", "environment.engine"]);
    let held: Vec<(&str, Vec<String>)> = artefact
        .points
        .into_iter()
        .map(|point| (point.name, point.occupied))
        .collect();
    assert_eq!(
        held,
        vec![
            (
                "doctor.check",
                vec!["environment.engine".to_owned(), "storage.space".to_owned()]
            ),
            ("doctor.remedy", Vec::new()),
        ]
    );
    assert_eq!(
        published(&[]).extension_points_version,
        EXTENSION_POINTS_VERSION
    );
}

/// Every point names the capability a contribution there asks for.
///
/// A contribution declares behaviour, and a build that read one it could not run
/// and dropped it would install a plugin whose declared behaviour is wider than its
/// actual one — so the name is published rather than left to an author to remember.
#[test]
fn every_point_names_the_capability_a_contribution_there_asks_for() {
    let asked: Vec<(&str, &str)> = published(&[])
        .points
        .into_iter()
        .map(|point| (point.name, point.requires))
        .collect();
    assert_eq!(
        asked,
        vec![
            ("doctor.check", "doctor.contribute"),
            ("doctor.remedy", "doctor.contribute"),
        ]
    );
}

/// Each table is keyed by the field it is about, and no field is in one twice.
///
/// The artefact keys both by field name, so a field written twice would produce a
/// document whose second entry silently replaced the first — which is a rule the
/// map form makes unnecessary for a reader and not for whoever writes the table.
#[test]
fn no_field_carries_two_bounds_or_two_closed_sets() {
    for point in POINTS {
        let bounded: BTreeSet<&str> = point.row.bounds.iter().map(|one| one.field).collect();
        let closed: BTreeSet<&str> = point.row.enums.iter().map(|one| one.field).collect();
        assert_eq!(bounded.len(), point.row.bounds.len(), "{}", point.name);
        assert_eq!(closed.len(), point.row.enums.len(), "{}", point.name);
    }
}

/// Both tables reach the artefact keyed by field, which is the shape it publishes.
#[test]
fn the_bounds_and_the_closed_sets_are_published_as_tables() {
    let text = published(&[])
        .points
        .iter()
        .filter_map(|point| serde_json::to_string(point).ok())
        .collect::<String>();
    assert!(text.contains(r#""bounds":{"timeout_s":{"#), "got: {text}");
    assert!(text.contains(r#""enums":{"category":["#), "got: {text}");
}

/// A row that declares no bound and no closed set says nothing about either.
///
/// The remedy's row has neither, and an artefact carrying two empty tables would be
/// publishing a shape nothing has — which a generator reading it would then have to
/// decide was meaningful.
#[test]
fn a_row_with_no_bounds_and_no_closed_sets_publishes_neither() {
    let text = published(&[])
        .points
        .iter()
        .filter(|point| point.name == "doctor.remedy")
        .filter_map(|point| serde_json::to_string(point).ok())
        .collect::<String>();
    assert!(!text.contains("bounds"), "got: {text}");
    assert!(!text.contains("enums"), "got: {text}");
}
