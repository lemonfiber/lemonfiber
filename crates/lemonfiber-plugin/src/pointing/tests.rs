use serde_json::json;

use super::{said, steps, Step};

/// The steps a key names, or the sentence it was refused with.
fn read(key: &str) -> Result<Vec<Step>, String> {
    steps(key).map_err(|why| why.0)
}

/// A name, which is every key written before pointers existed.
#[test]
fn a_key_that_is_not_a_pointer_is_the_name_of_a_top_level_member() {
    assert_eq!(read("content"), Ok(vec![Step::Named("content".to_owned())]));
    // A dot is part of the name and not a separator, which is what makes this
    // generation's reading of an old key the same as the last one's.
    assert_eq!(
        read("MediaContainer.size"),
        Ok(vec![Step::Named("MediaContainer.size".to_owned())])
    );
    // The empty string is the member named "", not the whole document: a key is a
    // name until it begins with a slash.
    assert_eq!(read(""), Ok(vec![Step::Named(String::new())]));
    // A bracket in a name is a name. The selector's rule is a pointer's rule.
    assert_eq!(read("a[0]"), Ok(vec![Step::Named("a[0]".to_owned())]));
}

#[test]
fn a_pointer_names_each_member_on_the_way_down() {
    assert_eq!(
        read("/MediaContainer/machineIdentifier"),
        Ok(vec![
            Step::Named("MediaContainer".to_owned()),
            Step::Named("machineIdentifier".to_owned()),
        ])
    );
    // One step, which is the same place a bare name reaches and says so differently.
    assert_eq!(
        read("/content"),
        Ok(vec![Step::Named("content".to_owned())])
    );
    // RFC 6901's own case: a pointer of one empty token is the member named "".
    assert_eq!(read("/"), Ok(vec![Step::Named(String::new())]));
}

#[test]
fn a_selector_picks_an_entry_of_a_list_by_a_field_it_holds() {
    assert_eq!(
        read("/MediaContainer/Setting/[id=PublishServerOnPlexOnlineKey]/value"),
        Ok(vec![
            Step::Named("MediaContainer".to_owned()),
            Step::Named("Setting".to_owned()),
            Step::Selected {
                field: "id".to_owned(),
                value: "PublishServerOnPlexOnlineKey".to_owned(),
            },
            Step::Named("value".to_owned()),
        ])
    );
}

/// A value may be empty, and a field may not.
///
/// Plex answers `"group": ""` for a hundred of its settings, so *the one whose group
/// is nothing* is a thing somebody will want to write. A field that is nothing is not
/// the same: there is no member to ask each entry for.
#[test]
fn a_selector_may_pick_by_a_field_holding_nothing_and_may_not_pick_by_no_field() {
    assert_eq!(
        read("/[group=]"),
        Ok(vec![Step::Selected {
            field: "group".to_owned(),
            value: String::new(),
        }])
    );
    let said = read("/[=x]").err().unwrap_or_default();
    assert!(said.contains("picks by no field"), "got: {said}");
    assert!(said.contains("[=x]"), "names the token: {said}");
}

/// A selector needs both halves, and the refusal shows the shape it wanted.
#[test]
fn a_selector_with_nothing_to_compare_is_refused_by_name() {
    let said = read("/Setting/[id]").err().unwrap_or_default();
    assert!(said.contains("names no field to pick by"), "got: {said}");
    assert!(said.contains("[field=value]"), "shows the shape: {said}");
    assert!(said.contains("[id]"), "names the token: {said}");
}

/// The suffix form the issue and the draft manifest both reach for.
///
/// `Setting[id=X]` reads as a member actually called that, which is a key nothing
/// answers and a mistake nothing would report — so a bracket in a reference token is
/// refused, and the refusal shows where the selector goes instead.
#[test]
fn a_selector_written_onto_a_member_name_is_refused_and_told_where_it_goes() {
    let said = read("/MediaContainer/Setting[id=X]/value")
        .err()
        .unwrap_or_default();
    assert!(said.contains("carries a bracket"), "got: {said}");
    assert!(said.contains("[field=value]"), "shows the shape: {said}");
    assert!(said.contains("Setting[id=X]"), "names the token: {said}");
    // The closing half alone is the same mistake and is refused the same way.
    assert!(read("/Setting]").is_err(), "a stray bracket is not a name");
}

/// A selector inside a selector is one step trying to be two.
#[test]
fn a_bracket_inside_a_selector_is_refused() {
    let said = read("/[id=[x]]").err().unwrap_or_default();
    assert!(said.contains("inside a selector"), "got: {said}");
}

/// RFC 6901's escapes, and the order they are read in.
///
/// `~01` is `~1` and not `/`, which is the case the two-replacement reading gets
/// wrong — and `~1~0` is `/~`, which is the case that shows the pass is doing both.
#[test]
fn the_two_escapes_are_read_in_one_pass_and_in_the_right_order() {
    assert_eq!(read("/a~1b"), Ok(vec![Step::Named("a/b".to_owned())]));
    assert_eq!(read("/a~0b"), Ok(vec![Step::Named("a~b".to_owned())]));
    assert_eq!(read("/~01"), Ok(vec![Step::Named("~1".to_owned())]));
    assert_eq!(read("/~1~0"), Ok(vec![Step::Named("/~".to_owned())]));
    // Both halves of a selector are read the same way, or a field with a slash in it
    // would be reachable in a member name and not in a selector.
    assert_eq!(
        read("/[a~1b=c~0d]"),
        Ok(vec![Step::Selected {
            field: "a/b".to_owned(),
            value: "c~d".to_owned(),
        }])
    );
}

/// An escape the RFC does not define is refused rather than read as a tilde.
#[test]
fn an_escape_that_is_not_one_is_refused_by_name() {
    let said = read("/a~2b").err().unwrap_or_default();
    assert!(said.contains("`~2`"), "names what it carries: {said}");
    assert!(said.contains("~0"), "names the escapes: {said}");
    let ends = read("/a~").err().unwrap_or_default();
    assert!(ends.contains("ends in a `~`"), "got: {ends}");
    // Inside a selector too, on either side of the comparison.
    assert!(read("/[a~2=b]").is_err(), "a field's escape is read");
    assert!(read("/[a=b~2]").is_err(), "a value's escape is read");
}

/// Every pointer without a selector reaches the place `serde_json` reaches.
///
/// The standard half of this is RFC 6901's and is not ours to re-specify, so it is
/// held to an implementation of the RFC that is already in the tree rather than to a
/// second reading of the document. Only the selector is this project's.
#[test]
fn the_standard_half_agrees_with_the_pointer_reader_already_here() {
    let document = json!({
        "": {"empty": 1},
        "a/b": 2,
        "a~b": 3,
        "MediaContainer": {"size": 0, "Setting": [{"id": "x", "value": false}]},
        "list": [10, 20],
    });
    for pointer in [
        "/MediaContainer",
        "/MediaContainer/size",
        "/MediaContainer/Setting",
        "/a~1b",
        "/a~0b",
        "//empty",
        "/nowhere",
        "/MediaContainer/nowhere",
        "/list",
    ] {
        let theirs = document.pointer(pointer);
        let ours = steps(pointer)
            .ok()
            .and_then(|steps| walked(&document, &steps));
        assert_eq!(ours, theirs, "{pointer}");
    }
}

/// The document at a place, for the agreement test above and nothing else.
///
/// Deliberately not the resolver a verdict is reached with: that one reports where it
/// stopped, and comparing two resolvers wants the plainer shape.
fn walked<'a>(document: &'a serde_json::Value, steps: &[Step]) -> Option<&'a serde_json::Value> {
    let mut at = document;
    for step in steps {
        match step {
            Step::Named(name) => at = at.as_object()?.get(name)?,
            Step::Selected { .. } => return None,
        }
    }
    Some(at)
}

/// The resolver that comparison is taken with stops at this project's own step, so
/// what is being agreed about is RFC 6901 and nothing beyond it.
///
/// The selector is the one part of a key no other implementation has, so there is
/// nothing to hold it to — and a helper that walked one anyway would quietly widen
/// the agreement above into a comparison of our own rule against itself.
#[test]
fn the_resolver_that_agreement_is_taken_with_reaches_no_selector() {
    let document = json!({"Setting": [{"id": "x", "value": false}]});
    let list = vec![Step::Named("Setting".to_owned())];
    assert_eq!(walked(&document, &list), document.pointer("/Setting"));
    let picked = vec![
        Step::Named("Setting".to_owned()),
        Step::Selected {
            field: "id".to_owned(),
            value: "x".to_owned(),
        },
    ];
    assert_eq!(walked(&document, &picked), None);
}

/// A key is written back as the author wrote it, so a refusal and a manifest agree.
#[test]
fn steps_are_written_back_as_the_key_that_named_them() {
    for key in [
        "/MediaContainer/machineIdentifier",
        "/MediaContainer/Setting/[id=PublishServerOnPlexOnlineKey]/value",
        "/a~1b/[c~0d=e]",
        "/",
    ] {
        let read = steps(key).unwrap_or_default();
        assert_eq!(said(&read), key, "{key}");
    }
    // A plain name is one step, and writing it back gives the pointer that names the
    // same member rather than the name itself — which is what a refusal should show
    // for a partial walk, and is why this is not the identity.
    assert_eq!(said(&[Step::Named("content".to_owned())]), "/content");
}
