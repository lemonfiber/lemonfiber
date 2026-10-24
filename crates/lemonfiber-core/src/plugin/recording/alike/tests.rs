use serde_json::json;

use super::alike;

#[test]
fn the_same_answer_is_alike() {
    let answer = json!({"status": 200, "body": {"name": "conformance", "count": 3}});
    assert!(alike(&answer, &answer));
}

#[test]
fn a_value_that_differs_is_not_alike() {
    assert!(!alike(
        &json!({"body": {"name": "conformance"}}),
        &json!({"body": {"name": "another"}})
    ));
}

#[test]
fn an_identifier_made_afresh_is_held_to_its_shape() {
    assert!(alike(
        &json!({"id": "3f2b8c1e-9a4d-4f6b-8e2a-1c5d7e9f0a3b"}),
        &json!({"id": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d"})
    ));
    assert!(alike(
        &json!(["c0f04a7e5e1d4b2a9c3f0e8d7b6a5c4d"]),
        &json!(["0123456789abcdef0123456789ABCDEF"])
    ));
}

#[test]
fn a_moment_made_afresh_is_held_to_its_shape() {
    assert!(alike(
        &json!({"at": "2026-10-24T05:12:33.1234567Z"}),
        &json!({"at": "2026-10-25T01:00:00+02:00"})
    ));
}

#[test]
fn one_shape_does_not_stand_for_another() {
    assert!(!alike(
        &json!("3f2b8c1e-9a4d-4f6b-8e2a-1c5d7e9f0a3b"),
        &json!("c0f04a7e5e1d4b2a9c3f0e8d7b6a5c4d")
    ));
    assert!(!alike(
        &json!("2026-10-24T05:12:33Z"),
        &json!("c0f04a7e5e1d4b2a9c3f0e8d7b6a5c4d")
    ));
    assert!(!alike(&json!("2026-10-24"), &json!("2026-10-25")));
}

#[test]
fn a_string_almost_in_a_shape_is_held_as_written() {
    assert!(!alike(
        &json!("3f2b8c1e-9a4d-4f6b-8e2a-1c5d7e9f0a3"),
        &json!("3f2b8c1e-9a4d-4f6b-8e2a-1c5d7e9f0a3c")
    ));
    assert!(!alike(
        &json!("g0f04a7e5e1d4b2a9c3f0e8d7b6a5c4d"),
        &json!("c0f04a7e5e1d4b2a9c3f0e8d7b6a5c4d")
    ));
}

#[test]
fn a_list_or_an_object_of_another_size_is_not_alike() {
    assert!(!alike(&json!([1, 2]), &json!([1, 2, 3])));
    assert!(!alike(&json!({"a": 1}), &json!({"a": 1, "b": 2})));
    assert!(!alike(&json!({"a": 1}), &json!({"b": 1})));
}

#[test]
fn values_of_another_kind_are_not_alike() {
    assert!(!alike(&json!("1"), &json!(1)));
    assert!(!alike(&json!(null), &json!(false)));
}
