use std::collections::BTreeSet;

use schemars::schema_for;
use serde_json::Value;

use super::super::tests::{names, said, INSTALLABLE};
use super::owner;
use crate::schema::Manifest;

/// The two declarations that account for everything the recipe below does.
///
/// Named apart from the recipe so a test can take one out without matching the
/// text of a whole block by hand, and so the text it takes out is by
/// construction the text the fixture carries.
const SECRET: &str = "\n[[secret]]\nid  = \"token\"\nof  = \"komga\"\n\
                          why = \"The session the sign-in hands back\"\n";

/// The one bundled thing it says it will change.
const OVERRIDE: &str = "\n[[override]]\nid  = \"homepage.services\"\n\
                            why = \"Add its own entry to the bundled dashboard\"\n";

/// A recipe this build reads and refuses to run, declaring everything it reaches.
///
/// Written here rather than taken from the whole-format fixture, because this
/// one has a job that fixture cannot do: it is the *accepted* shape. Every value
/// it captures is declared and the one bundled thing it changes is declared, so
/// the only thing this build has to say about it is that it cannot run a recipe
/// at all — which is what lets each test below remove exactly one declaration
/// and watch the refusal for that one, and only that one, appear.
const CALLS: &str = r#"
[[recipe]]
id    = "adopt-existing-library"
title = "Point it at the comics the stack already files"
why   = "The stack already files comics, and a fresh Komga knows nothing about them."

[[recipe.step]]
id      = "sign-in"
call    = { method = "POST", to = "komga", path = "/api/v1/login" }
expect  = { status = 200 }
capture = [{ name = "token", from = "json.token", origin = "stack-service" }]

[[recipe.step]]
id      = "announce"
call    = { method = "POST", to = "homepage", path = "/api/services", headers = { Authorization = "Bearer {{token}}" } }
expect  = { status = 200 }

[[recipe.pair]]
value = "token"
to    = "homepage"
"#;

/// The installable manifest, with a recipe and its declarations on the end.
fn declaring(recipe: &str) -> String {
    let asked = INSTALLABLE.replace(
        r#"capabilities = ["doctor.contribute"]"#,
        r#"capabilities = ["doctor.contribute", "recipe.run"]"#,
    );
    format!("{asked}{recipe}")
}

/// The whole fixture: the calls, the secret and the override.
fn whole() -> String {
    declaring(&format!("{CALLS}{SECRET}{OVERRIDE}"))
}

/// What this build says about the fixture with one piece of it edited.
fn edited(before: &str, after: &str) -> Vec<String> {
    let text = whole();
    assert!(text.contains(before), "the fixture still says {before:?}");
    said(&text.replace(before, after))
}

/// The fixture is refused for the one reason it is meant to be refused for.
///
/// The acceptance side of both rules at once, and it is asserted rather than
/// assumed because a rule that refuses everything it is shown passes every test
/// written as *this is refused*. What this build has to say about a recipe whose
/// captures and changes are all declared is that it cannot run one, and nothing
/// else.
#[test]
fn a_recipe_whose_every_reach_is_declared_is_refused_only_for_being_a_recipe() {
    let said = said(&whole());
    assert!(
        names(&said, &["requires.capabilities", "recipe.run"]),
        "got: {said:?}"
    );
    assert_eq!(said.len(), 1, "and nothing else is said: {said:?}");
}

#[test]
fn a_value_captured_that_no_secret_declares_is_refused_naming_it_and_its_origin() {
    let said = edited(r#"id  = "token""#, r#"id  = "api-key""#);
    assert!(
        names(
            &said,
            &[
                "recipe adopt-existing-library.step sign-in.capture",
                "token",
                "stack-service",
                "[[secret]]",
            ]
        ),
        "got: {said:?}"
    );
}

/// Declaring nothing at all is the same reach, written a second way.
#[test]
fn a_recipe_that_captures_and_declares_no_secret_is_refused() {
    let said = edited(SECRET, "");
    assert!(names(&said, &["token", "[[secret]]"]), "got: {said:?}");
}

#[test]
fn a_bundled_thing_changed_that_no_override_declares_is_refused_naming_both() {
    let said = edited(r#"id  = "homepage.services""#, r#"id  = "komga.libraries""#);
    assert!(
        names(
            &said,
            &[
                "recipe adopt-existing-library.step announce.call",
                "/api/services",
                "homepage",
                "[[override]]",
            ]
        ),
        "got: {said:?}"
    );
}

/// A call that writes needs the change declared; one that only asks does not.
///
/// Asked of every verb the format admits rather than of one, and both
/// directions in one assertion, because the halves fail separately: a rule that
/// refuses every call would pass a test that only shows a write refused, and a
/// rule that refuses none would pass one that only shows a read accepted.
///
/// The classification is written as a complement, so a verb admitted to the
/// format and never thought about here is read as a change — which asks its
/// author for a declaration they may not have needed, rather than letting a
/// bundled setting be changed with nobody told.
#[test]
fn a_call_that_writes_needs_the_change_declared_and_one_that_only_asks_does_not() {
    for verb in super::super::recipes::METHODS {
        let undeclared = whole().replace(OVERRIDE, "").replace(
            r#"{ method = "POST", to = "homepage""#,
            &format!(r#"{{ method = "{verb}", to = "homepage""#),
        );
        let said = said(&undeclared);
        assert_eq!(
            names(&said, &["announce.call", "[[override]]"]),
            !super::ASKS.contains(verb),
            "{verb} against an undeclared bundled service: {said:?}"
        );
    }
    assert!(
        super::ASKS
            .iter()
            .all(|verb| super::super::recipes::METHODS.contains(verb)),
        "a verb read as one that only asks is one a call may actually use"
    );
}

/// A plugin changing what it installed is changing its own, not the bundle's.
///
/// With the declaration taken out, so the bundled call in the same recipe is
/// refused in the same answer. Without that half the test would pass against a
/// rule that had never run.
#[test]
fn a_change_to_the_plugins_own_service_needs_no_override() {
    let said = edited(OVERRIDE, "");
    assert!(
        names(&said, &["step announce.call", "[[override]]"]),
        "the bundled call in the same recipe is refused: {said:?}"
    );
    assert!(
        !names(&said, &["step sign-in.call", "[[override]]"]),
        "and the call to its own service is not: {said:?}"
    );
}

/// And a call that leaves the stack is declared as a host rather than as one.
///
/// The same shape: the call is shown refused where it goes to a bundled
/// service, and then not refused where the only thing changed about it is that
/// it goes somewhere the stack does not ship.
#[test]
fn a_call_to_a_name_outside_the_stack_is_not_an_override() {
    let inside = whole().replace(OVERRIDE, "");
    assert!(
        names(&said(&inside), &["announce.call", "[[override]]"]),
        "the bundled destination is refused"
    );

    let outside = inside
        .replace(r#"to = "homepage""#, r#"to = "plex.tv""#)
        .replace(r#"to    = "homepage""#, r#"to    = "plex.tv""#);
    let said = said(&outside);
    assert!(!names(&said, &["[[override]]"]), "got: {said:?}");
}

/// A declaration names an owner and what of theirs, and the owner is read off it.
#[test]
fn whose_a_declaration_is_about_is_the_name_before_the_first_stop() {
    assert_eq!(owner("homepage.services"), "homepage");
    assert_eq!(owner("homepage.services.first"), "homepage");
    assert_eq!(owner("homepage"), "homepage");
}

/// A manifest with no recipe in it reaches past nothing, and is told nothing.
#[test]
fn a_manifest_that_captures_and_changes_nothing_is_refused_neither() {
    let said = said(INSTALLABLE);
    assert!(!names(&said, &["[[secret]]"]), "got: {said:?}");
    assert!(!names(&said, &["[[override]]"]), "got: {said:?}");
}

/// Where the published schema lets a manifest take a value out of an answer.
///
/// The sweep above walks one path, and one path is the whole of the format only
/// for as long as the format has one. A second place to capture — a widget
/// reading a service's API, an adapter handing back what it found — would leave
/// the rule true of the path it knows and silent about the new one, and nothing
/// about the rule itself would look wrong. Asked of the schema, which is
/// generated from the types the reader deserialises and so cannot describe a
/// format the reader does not have.
#[test]
fn the_only_place_a_value_can_be_captured_is_the_one_this_sweeps() {
    assert_eq!(sites("Capture"), vec!["recipe.step.capture".to_owned()]);
}

/// And where it lets one make a call, which is the other half.
#[test]
fn the_only_place_a_call_can_be_made_is_the_one_this_sweeps() {
    assert_eq!(sites("StepCall"), vec!["recipe.step.call".to_owned()]);
}

/// A request outside a recipe cannot say where it is asking, so it asks the
/// plugin's own service and there is no field by which it could ask anywhere
/// else.
///
/// That is what keeps the change rule's three destinations at three. A proof or
/// a contributed check able to name where it went would be a call to a bundled
/// service wearing another block's name, with no pair behind it and no override
/// in front of it.
#[test]
fn nothing_outside_a_recipe_can_name_where_it_is_going() {
    let reaching: Vec<&str> = ["to", "host", "url", "address", "origin", "server"]
        .into_iter()
        .filter(|field| carries("PluginRequest", field))
        .collect();
    assert!(
        reaching.is_empty(),
        "a request outside a recipe names where it goes, which is a call with no pair \
             behind it: {reaching:?}"
    );
}

/// The walk is shown finding a second site, on a schema planted to have two.
///
/// Without this the two gates above are a measurement nobody has watched
/// succeed: a walk that always answered with one path would satisfy them both
/// and would notice nothing it exists to notice.
#[test]
fn the_walk_finds_every_site_rather_than_the_first() {
    let planted: Value = serde_json::from_str(
        r##"{
                "properties": {
                    "recipe": { "$ref": "#/$defs/Recipe" },
                    "widget": { "$ref": "#/$defs/Widget" }
                },
                "$defs": {
                    "Recipe": {
                        "properties": {
                            "capture": { "items": { "$ref": "#/$defs/Capture" } }
                        }
                    },
                    "Widget": { "properties": { "reads": { "$ref": "#/$defs/Capture" } } },
                    "Capture": { "properties": {} }
                }
            }"##,
    )
    .unwrap_or(Value::Null);
    assert_eq!(
        walked(&planted, "Capture"),
        vec!["recipe.capture".to_owned(), "widget.reads".to_owned()]
    );
    assert!(walked(&planted, "Nothing").is_empty());
}

/// A schema that refers to itself comes back, and one that refers to nothing
/// finds no path through it.
///
/// The two ways this walk could fail to answer, and neither is reachable from
/// the schema this build publishes — which is the whole reason they are planted.
/// The bound below says in words that it exists so a definition which one day
/// refers to itself is not a hang; a bound nothing has ever reached is a claim
/// rather than a measurement. And a reference naming a definition that is not
/// there is what a schema mid-rename looks like: it must be no path rather than
/// a panic or a walk that stops early with the sites before it.
#[test]
fn a_schema_that_refers_to_itself_is_bounded_and_one_that_refers_to_nothing_finds_no_path() {
    let circular: Value = serde_json::from_str(
        r##"{
                "properties": { "step": { "$ref": "#/$defs/Step" } },
                "$defs": {
                    "Step": {
                        "properties": {
                            "capture": { "$ref": "#/$defs/Capture" },
                            "then": { "$ref": "#/$defs/Step" }
                        }
                    },
                    "Capture": { "properties": {} }
                }
            }"##,
    )
    .unwrap_or(Value::Null);
    let found = walked(&circular, "Capture");
    assert_eq!(
        found.len(),
        DEEP,
        "one path for each depth the bound allows, and then it stops: {found:?}"
    );
    assert_eq!(found.first().map(String::as_str), Some("step.capture"));

    let dangling: Value = serde_json::from_str(
        r##"{
                "properties": { "step": { "$ref": "#/$defs/Gone" } },
                "$defs": { "Capture": { "properties": {} } }
            }"##,
    )
    .unwrap_or(Value::Null);
    assert!(walked(&dangling, "Capture").is_empty());
}

/// Every path in the published schema at which one declaration can appear.
fn sites(of: &str) -> Vec<String> {
    walked(&published(), of)
}

/// Whether one declaration in the published schema carries a field by this name.
fn carries(of: &str, field: &str) -> bool {
    published()
        .pointer(&format!("/$defs/{of}/properties/{field}"))
        .is_some()
}

/// The schema this build publishes, as a document to walk.
fn published() -> Value {
    serde_json::to_value(schema_for!(Manifest)).unwrap_or(Value::Null)
}

/// Where one definition is reachable from, by the path a manifest writes it at.
fn walked(schema: &Value, of: &str) -> Vec<String> {
    let defs = schema.get("$defs").cloned().unwrap_or(Value::Null);
    let mut found = Vec::new();
    reached(schema, &defs, "", of, 0, &mut found);
    found
}

/// How deep a manifest's declarations nest, with room to spare.
///
/// A bound rather than a set of what has been seen, because the question is
/// about paths and a definition reachable twice is two of them. Nothing in this
/// format refers to itself; the bound is what keeps that from being a hang if
/// something one day does.
const DEEP: usize = 8;

fn reached(at: &Value, defs: &Value, path: &str, of: &str, depth: usize, found: &mut Vec<String>) {
    if depth > DEEP {
        return;
    }
    let Some(properties) = at.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (name, child) in properties {
        let here = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}.{name}")
        };
        for named in referenced(child) {
            if named == of {
                found.push(here.clone());
            }
            if let Some(next) = defs.get(&named) {
                reached(next, defs, &here, of, depth + 1, found);
            }
        }
    }
}

/// Every definition one property's schema names, however it is wrapped.
///
/// A field is a list, or optional, or both, and each of those is a layer of
/// JSON Schema around the reference rather than a different reference. Read by
/// gathering every reference under the property, so a wrapper this does not
/// know about cannot make the definition inside it invisible.
fn referenced(value: &Value) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    gather(value, &mut found);
    found
        .into_iter()
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}

/// The recursive half, which takes a reference wherever one is written.
fn gather(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            for (name, held) in fields {
                match held.as_str() {
                    Some(named) if name == "$ref" => {
                        found.push(named.rsplit('/').next().unwrap_or(named).to_owned());
                    }
                    _ => gather(held, found),
                }
            }
        }
        Value::Array(each) => {
            for held in each {
                gather(held, found);
            }
        }
        _ => {}
    }
}
