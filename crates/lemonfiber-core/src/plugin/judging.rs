//! Whether an answer is the one an assertion declared.
//!
//! One evaluator for the three places an expectation is written — a claim's probe, a
//! proof and a contributed check — because all three ask a service a question and judge
//! what came back. Three evaluators for one vocabulary would be three things to keep in
//! step, and the one that fell behind would be quietly checking less than it said.
//!
//! Every way the answer is not the declared one is reported, rather than the first:
//! whoever is looking at this is holding a recording and a manifest and deciding which
//! of them is wrong, and one fault at a time makes that a guessing game.

use lemonfiber_plugin::{Expect, Expected, Kind};
use serde_json::Value;

use super::recorded::Answer;

/// Every way this answer is not the one the expectation declared.
///
/// An empty answer is the assertion holding.
#[must_use]
pub fn judge(expect: &Expect, answer: &Answer) -> Vec<String> {
    let mut faults = Vec::new();
    if let Some(wanted) = expect.status {
        if answer.status != wanted {
            faults.push(format!(
                "answered {} where it declares {wanted}",
                answer.status
            ));
        }
    }
    exact(expect, answer, &mut faults);
    keys(expect, answer, &mut faults);
    shape(expect, answer, &mut faults);
    served(expect, answer, &mut faults);
    faults
}

/// The keys an answer must carry with the exact value it must hold.
fn exact(expect: &Expect, answer: &Answer, faults: &mut Vec<String>) {
    let Some(wanted) = &expect.json else { return };
    for (key, value) in wanted {
        match held(answer, key) {
            None => faults.push(format!("the body carries no {key}")),
            Some(found) if !same(value, found) => {
                faults.push(format!("{key} is {found}, and it declares {}", said(value)));
            }
            Some(_) => {}
        }
    }
}

/// The keys an answer must carry, whatever they hold, and the kinds they must be.
fn keys(expect: &Expect, answer: &Answer, faults: &mut Vec<String>) {
    for key in expect.json_has_keys.iter().flatten() {
        if held(answer, key).is_none() {
            faults.push(format!("the body carries no {key}"));
        }
    }
    for (key, kind) in expect.json_types.iter().flatten() {
        match held(answer, key) {
            None => faults.push(format!("the body carries no {key}")),
            Some(found) if !is_kind(found, *kind) => faults.push(format!(
                "{key} is {}, and it declares {}",
                kind_of(found),
                kind_said(*kind)
            )),
            Some(_) => {}
        }
    }
    for (key, least) in expect.json_at_least.iter().flatten() {
        match held(answer, key).and_then(Value::as_i64) {
            None => faults.push(format!("the body carries no number at {key}")),
            Some(found) if found < *least => {
                faults.push(format!(
                    "{key} is {found}, and it declares at least {least}"
                ));
            }
            Some(_) => {}
        }
    }
}

/// What the body is as a whole: a list of a given length, or not a document at all.
fn shape(expect: &Expect, answer: &Answer, faults: &mut Vec<String>) {
    if let Some(least) = expect.json_array_min {
        match answer.json.as_ref().and_then(Value::as_array) {
            None => faults.push("the body is not a list".to_owned()),
            Some(entries) if (entries.len() as u64) < least => faults.push(format!(
                "the list holds {}, and it declares at least {least}",
                entries.len()
            )),
            Some(_) => {}
        }
    }
    if expect.json_is_absent == Some(true) && answer.json.is_some() {
        faults.push("the body parsed as a document, and it declares that it does not".to_owned());
    }
}

/// What the answer was served as, and what it begins with where it is not a document.
fn served(expect: &Expect, answer: &Answer, faults: &mut Vec<String>) {
    if let Some(wanted) = &expect.content_type {
        let served = answer
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .map_or("", |(_, value)| value.as_str());
        if !served.contains(wanted.as_str()) {
            faults.push(format!(
                "it was served as {served:?}, and it declares {wanted:?}"
            ));
        }
    }
    if let Some(wanted) = &expect.body_starts_with {
        let begins = answer.body_starts_with.as_deref().unwrap_or_default();
        if !begins.starts_with(wanted.as_str()) {
            faults.push(format!(
                "the body begins {:?}, and it declares {wanted:?}",
                begins.chars().take(40).collect::<String>()
            ));
        }
    }
}

/// What the body holds at a key, where the body is an object that has one.
fn held<'a>(answer: &'a Answer, key: &str) -> Option<&'a Value> {
    answer.json.as_ref()?.as_object()?.get(key)
}

/// Whether a value is the one an expectation names.
fn same(wanted: &Expected, found: &Value) -> bool {
    match wanted {
        Expected::Flag(flag) => found.as_bool() == Some(*flag),
        Expected::Number(number) => found.as_i64() == Some(*number),
        Expected::Word(word) => found.as_str() == Some(word.as_str()),
    }
}

/// An expectation's value, as a refusal spells it.
fn said(wanted: &Expected) -> String {
    match wanted {
        Expected::Flag(flag) => flag.to_string(),
        Expected::Number(number) => number.to_string(),
        Expected::Word(word) => format!("{word:?}"),
    }
}

/// Whether a value is of the kind an expectation names.
fn is_kind(found: &Value, kind: Kind) -> bool {
    match kind {
        Kind::Bool => found.is_boolean(),
        Kind::Int => found.is_i64() || found.is_u64(),
        Kind::Str => found.is_string(),
        Kind::List => found.is_array(),
        Kind::Dict => found.is_object(),
    }
}

/// What kind a value actually is, in the words the expectation uses for them.
fn kind_of(found: &Value) -> &'static str {
    match found {
        Value::Bool(_) => "a true or a false",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        Value::Object(_) => "an object",
        Value::Null => "nothing",
    }
}

/// The kind an expectation named, in the same words.
fn kind_said(kind: Kind) -> &'static str {
    match kind {
        Kind::Bool => "a true or a false",
        Kind::Int => "a number",
        Kind::Str => "a string",
        Kind::List => "a list",
        Kind::Dict => "an object",
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber_plugin::Expect;

    use super::super::recorded::{Answer, Recording};
    use super::judge;

    /// An answer built out of a recording, which is the only way one is ever built.
    ///
    /// Through the recording's own reader rather than by constructing the struct, so a
    /// test cannot assert about a shape a recording could not produce.
    fn answered(response: &str) -> Answer {
        let text = format!(
            r#"{{"recorded_from": "example.invalid/a@sha256:{}", "note": "n",
                "request": {{"method": "GET", "path": "/"}}, "response": {response}}}"#,
            "0".repeat(64)
        );
        serde_json::from_str::<Recording>(&text)
            .map(|recording| recording.response)
            .unwrap_or_default()
    }

    /// An expectation, read the way a manifest's is: through its own deserialiser.
    ///
    /// One that does not read comes back saying nothing, which fails the assertion
    /// under it rather than ending the run — and leaves no arm a test cannot enter.
    fn expects(declared: &str) -> Expect {
        serde_json::from_str(declared).unwrap_or_default()
    }

    #[test]
    fn an_answer_that_is_the_declared_one_has_nothing_wrong_with_it() {
        let faults = judge(
            &expects(r#"{"status": 401}"#),
            &answered(r#"{"status": 401, "headers": {"content-type": "application/json"}}"#),
        );
        assert_eq!(faults, Vec::<String>::new());
    }

    #[test]
    fn a_status_that_is_not_the_declared_one_says_both() {
        let faults = judge(
            &expects(r#"{"status": 401}"#),
            &answered(r#"{"status": 200}"#),
        );
        assert_eq!(faults, vec!["answered 200 where it declares 401"]);
    }

    /// The whole list, because whoever is reading it is deciding which of the manifest
    /// and the recording is wrong, and one fault at a time makes that a guessing game.
    #[test]
    fn every_way_the_answer_falls_short_is_reported_at_once() {
        let faults = judge(
            &expects(r#"{"status": 200, "json_has_keys": ["a", "b"]}"#),
            &answered(r#"{"status": 404, "json": {}}"#),
        );
        assert_eq!(faults.len(), 3, "got: {faults:?}");
    }

    #[test]
    fn a_key_holding_something_else_names_what_it_holds() {
        let faults = judge(
            &expects(r#"{"json": {"isClaimed": true}}"#),
            &answered(r#"{"status": 200, "json": {"isClaimed": false}}"#),
        );
        assert_eq!(faults, vec!["isClaimed is false, and it declares true"]);
    }

    #[test]
    fn a_key_of_the_wrong_kind_names_both_kinds() {
        let faults = judge(
            &expects(r#"{"json_types": {"type": "str"}}"#),
            &answered(r#"{"status": 200, "json": {"type": 3}}"#),
        );
        assert_eq!(faults, vec!["type is a number, and it declares a string"]);
    }

    /// Every kind an expectation can name, against a body holding each of them.
    ///
    /// A table rather than a case each, because what is being held is a mapping: the
    /// five words a manifest may write and the six shapes a body can come back as. A
    /// kind this could not tell apart would let a proof pass on the wrong sort of value.
    #[test]
    fn every_kind_is_told_from_every_other() {
        let holding = [
            ("bool", r#"{"a": true}"#, "a true or a false"),
            ("int", r#"{"a": 3}"#, "a number"),
            ("str", r#"{"a": "x"}"#, "a string"),
            ("list", r#"{"a": []}"#, "a list"),
            ("dict", r#"{"a": {}}"#, "an object"),
        ];
        for (kind, body, said) in holding {
            let answer = answered(&format!(r#"{{"status": 200, "json": {body}}}"#));
            let right = expects(&format!(r#"{{"json_types": {{"a": "{kind}"}}}}"#));
            assert_eq!(judge(&right, &answer), Vec::<String>::new(), "{kind} holds");

            for (other, _, wanted) in holding {
                if other == kind {
                    continue;
                }
                let wrong = expects(&format!(r#"{{"json_types": {{"a": "{other}"}}}}"#));
                assert_eq!(
                    judge(&wrong, &answer),
                    vec![format!("a is {said}, and it declares {wanted}")],
                    "{kind} read as {other}"
                );
            }
        }
        // The sixth shape a body can come back as, which no kind names: a key that is
        // there and holds nothing is not a key that is missing.
        let nothing = answered(r#"{"status": 200, "json": {"a": null}}"#);
        assert_eq!(
            judge(&expects(r#"{"json_types": {"a": "str"}}"#), &nothing),
            vec!["a is nothing, and it declares a string"]
        );
    }

    /// The three things a key may be declared to hold, each named as it was written.
    #[test]
    fn a_flag_a_number_and_a_word_are_each_said_back_as_they_were_declared() {
        let answer =
            answered(r#"{"status": 200, "json": {"flag": false, "count": 1, "word": "no"}}"#);
        let faults = judge(
            &expects(r#"{"json": {"flag": true, "count": 2, "word": "yes"}}"#),
            &answer,
        );
        assert_eq!(
            faults,
            vec![
                "count is 1, and it declares 2",
                "flag is false, and it declares true",
                "word is \"no\", and it declares \"yes\"",
            ]
        );
    }

    #[test]
    fn a_number_below_what_was_declared_says_both() {
        let faults = judge(
            &expects(r#"{"json_at_least": {"total": 2}}"#),
            &answered(r#"{"status": 200, "json": {"total": 1}}"#),
        );
        assert_eq!(faults, vec!["total is 1, and it declares at least 2"]);
    }

    #[test]
    fn a_body_that_is_not_a_list_cannot_answer_a_length() {
        let faults = judge(
            &expects(r#"{"json_array_min": 1}"#),
            &answered(r#"{"status": 200, "json": {"content": []}}"#),
        );
        assert_eq!(faults, vec!["the body is not a list"]);
        let short = judge(
            &expects(r#"{"json_array_min": 2}"#),
            &answered(r#"{"status": 200, "json": [1]}"#),
        );
        assert_eq!(short, vec!["the list holds 1, and it declares at least 2"]);
    }

    /// The constraint whose name invites the other reading: it says nothing about keys,
    /// it says the answer was not a document at all.
    #[test]
    fn a_body_that_parsed_fails_an_expectation_that_it_would_not() {
        let faults = judge(
            &expects(r#"{"json_is_absent": true}"#),
            &answered(r#"{"status": 200, "json": {"anything": 1}}"#),
        );
        assert_eq!(
            faults,
            vec!["the body parsed as a document, and it declares that it does not"]
        );
        let shell = judge(
            &expects(r#"{"json_is_absent": true}"#),
            &answered(r#"{"status": 200, "json": null, "body_starts_with": "<!DOCTYPE html>"}"#),
        );
        assert_eq!(shell, Vec::<String>::new());
    }

    /// The pair an application shell is caught by: it answered 200 with a page, and the
    /// only honest evidence is that the body was not a document and began as HTML.
    #[test]
    fn a_body_that_is_not_a_document_is_judged_by_what_it_begins_with() {
        let expect = expects(
            r#"{"status": 200, "content_type": "text/html", "json_is_absent": true,
                "body_starts_with": "<!DOCTYPE html>"}"#,
        );
        let shell = answered(
            r#"{"status": 200, "headers": {"content-type": "text/html; charset=utf-8"},
                "json": null, "body_starts_with": "<!DOCTYPE html>\n<html lang=\"en\">"}"#,
        );
        assert_eq!(judge(&expect, &shell), Vec::<String>::new());
    }

    #[test]
    fn a_content_type_it_was_not_served_as_names_both() {
        let faults = judge(
            &expects(r#"{"content_type": "application/opds-authentication+json"}"#),
            &answered(r#"{"status": 401, "headers": {"content-type": "application/json"}}"#),
        );
        assert_eq!(faults.len(), 1, "got: {faults:?}");
        assert!(
            faults
                .first()
                .is_some_and(|said| said.contains("application/json")
                    && said.contains("opds-authentication")),
            "got: {faults:?}"
        );
    }

    /// A header name is not case-sensitive on the wire, and a recording keeps whatever
    /// the service sent — so a capital letter in one must not decide an expectation.
    #[test]
    fn the_content_type_is_found_however_the_recording_spells_the_header() {
        let faults = judge(
            &expects(r#"{"content_type": "application/json"}"#),
            &answered(r#"{"status": 200, "headers": {"Content-Type": "application/json"}}"#),
        );
        assert_eq!(faults, Vec::<String>::new());
    }

    /// A key the body does not carry is the same fault whichever constraint asked for
    /// it, and every one of them says so rather than passing over a missing key.
    #[test]
    fn a_key_that_is_not_there_is_named_by_every_constraint_that_wanted_it() {
        let expect = expects(
            r#"{"json": {"a": 1}, "json_has_keys": ["b"], "json_types": {"c": "int"},
                "json_at_least": {"d": 1}}"#,
        );
        let faults = judge(&expect, &answered(r#"{"status": 200, "json": {}}"#));
        assert_eq!(faults.len(), 4, "got: {faults:?}");
    }

    /// An expectation that says nothing judges nothing, which is why the vocabulary
    /// refuses a claim that carries one rather than this quietly passing it.
    #[test]
    fn an_expectation_that_declares_nothing_finds_nothing_wrong() {
        assert_eq!(
            judge(&expects("{}"), &answered(r#"{"status": 500}"#)),
            Vec::<String>::new()
        );
    }
}
