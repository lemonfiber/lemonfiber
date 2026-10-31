use serde_json::json;

use super::{held, place, shape, SHAPE_MOST};

#[test]
fn a_shape_matches_a_whole_string_and_nothing_else() {
    let forty = "[0-9a-f]{40}";
    let identifier = json!("946d83ffdbc9067a9ff34de253bff49bec92708f");
    assert!(held(forty, Some(&identifier)));
    assert!(!held(
        forty,
        Some(&json!("946d83ffdbc9067a9ff34de253bff49bec92708f0"))
    ));
    assert!(!held(
        forty,
        Some(&json!("x946d83ffdbc9067a9ff34de253bff49bec92708"))
    ));
    assert!(!held(forty, Some(&json!(40))));
    assert!(!held(forty, None));
    assert!(!held("[", Some(&identifier)));
}

#[test]
fn a_shape_that_is_too_long_or_not_a_pattern_is_refused() {
    assert!(shape("[0-9a-f]{40}").is_ok());
    assert!(shape("(").is_err_and(|why| why.contains("not a pattern this build compiles")));
    assert!(shape(&"a".repeat(SHAPE_MOST + 1)).is_err_and(|why| why.contains("longer than")));
    assert!(shape("a{1000}{1000}").is_err());
}

#[test]
fn a_place_is_a_pointer_naming_a_member_at_each_step() {
    assert!(place("/MediaContainer/machineIdentifier").is_ok());
    assert!(place("machineIdentifier").is_err_and(|why| why.contains("not a JSON Pointer")));
    assert!(place("/Setting/[id=Key]/value").is_err_and(|why| why.contains("picks a list")));
    assert!(place("/a~2").is_err());
}
