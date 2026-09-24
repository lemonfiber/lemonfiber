use toml::Value;

use super::{unrecognised, DECLARED, ON_PROFILE, ON_SERVICE};
use crate::names::{scan, Closed};
use crate::Manifest;

/// A word no enumeration in the contract will ever hold.
const UNKNOWABLE: &str = "zzz-not-a-name";

#[test]
fn names_the_field_and_the_entry_it_was_declared_on() {
    let found = unrecognised(
        "
[[service]]
id = \"jellyfin\"
api = { kind = \"plex\", key_source = \"config-xml\" }
",
    );
    let said: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert_eq!(said.len(), 1, "one refusal: {said:?}");
    assert!(
        said.first()
            .is_some_and(|one| one.contains("service jellyfin")
                && one.contains("api.kind")
                && one.contains("`plex`")),
        "got: {said:?}"
    );
}

#[test]
fn an_entry_with_no_id_yet_is_still_placed() {
    let found = unrecognised(
        "
[[profile]]
protocol = \"carrier-pigeon\"
",
    );
    assert!(
        found.first().is_some_and(|one| one.location == "profile 0"),
        "got: {found:?}"
    );
}

#[test]
fn a_field_holding_something_other_than_a_name_is_left_to_the_reader() {
    let found = unrecognised(
        "
[[service]]
id = \"q\"
criticality = 3
",
    );
    assert!(
        found.is_empty(),
        "a number is not a name it refused: {found:?}"
    );
}

#[test]
fn a_file_that_is_not_toml_is_left_to_the_reader() {
    assert!(unrecognised("= not toml").is_empty());
}

/// Swap the `want`th string in the tree, counting through `seen` as it goes.
fn nth_word(value: &mut Value, want: usize, seen: &mut usize) {
    match value {
        Value::String(word) => {
            if *seen == want {
                UNKNOWABLE.clone_into(word);
            }
            *seen += 1;
        }
        Value::Array(items) => items.iter_mut().for_each(|item| nth_word(item, want, seen)),
        Value::Table(fields) => fields
            .iter_mut()
            .for_each(|(_, field)| nth_word(field, want, seen)),
        _ => {}
    }
}

/// What the typed read said about one swapped word the scan did not name.
///
/// Asked of the read rather than of `from_toml`, because `from_toml` consults the
/// whole table and would name what a short one missed — which is the answer this
/// is trying to tell apart.
fn escaped_at(tree: &Value, nth: usize, declared: &[(&str, &[Closed])]) -> Option<String> {
    let mut copy = tree.clone();
    nth_word(&mut copy, nth, &mut 0);
    let text = toml::to_string(&copy).ok()?;
    if !scan(&text, declared).is_empty() {
        return None;
    }
    toml::from_str::<Manifest>(&text)
        .err()
        .map(|said| said.to_string())
        .filter(|said| said.contains("unknown variant"))
}

/// Every word in a manifest, swapped in turn for one no enumeration holds.
fn swept(tree: &Value, declared: &[(&str, &[Closed])]) -> (usize, Vec<String>) {
    let mut words = 0;
    nth_word(&mut tree.clone(), usize::MAX, &mut words);
    let escaped = (0..words)
        .filter_map(|nth| escaped_at(tree, nth, declared))
        .collect();
    (words, escaped)
}

/// The shipped stack, and what a sweep of it found a given table failing to name.
fn sweeping(declared: &[(&str, &[Closed])]) -> (usize, Vec<String>) {
    let embedded = include_str!("../../../../assets/media-stack/stack.toml");
    toml::from_str::<Value>(embedded)
        .map(|tree| swept(&tree, declared))
        .unwrap_or_default()
}

/// The table above is a list of paths, and a list is a thing that goes stale.
///
/// Adding a field whose type is a closed set and forgetting to list it here is
/// invisible: the manifest still refuses the unknown name, just as the parse
/// failure this module exists to replace. So every word the shipped stack
/// declares is swapped in turn for one nothing can accept, and the only answer
/// allowed is a refusal that named it — never one that failed to parse.
#[test]
fn no_closed_field_in_the_shipped_stack_is_left_off_the_table() {
    let (words, escaped) = sweeping(DECLARED);
    assert!(words > 100, "the shipped stack was read and swept: {words}");
    assert!(
        escaped.is_empty(),
        "a closed field is missing from the table above, so an unknown name \
             came back as a parse failure: {escaped:?}"
    );
}

/// And the sweep can fail, which is the half a passing sweep says nothing about.
///
/// A gate nobody has watched refuse is a gate nobody knows works: run against a
/// table with the last service field taken off it, every service declaring that
/// field has to come back unnamed.
#[test]
fn a_field_left_off_the_table_is_what_the_sweep_refuses() {
    let short = ON_SERVICE.split_last().map_or(&[][..], |(_, rest)| rest);
    let (_, escaped) = sweeping(&[("profile", ON_PROFILE), ("service", short)]);
    assert!(
        escaped.iter().any(|said| said.contains("key_source")),
        "a table missing api.key_source lets it through as a parse failure: \
             {escaped:?}"
    );
}
