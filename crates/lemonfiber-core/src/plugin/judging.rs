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

use lemonfiber_plugin::pointing::{self, Step};
use lemonfiber_plugin::{Expect, Expected, Kind};
use serde_json::Value;

use super::recorded::Answer;

/// The most of a value a refusal prints before it stops being readable.
///
/// A refusal is read by somebody holding a manifest and a recording and deciding which
/// of the two is wrong. Plex answers a hundred and fifty-one settings at `/:/prefs`, and
/// a refusal that printed all of them said everything and showed nothing.
const READABLE: usize = 120;

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

/// The places an answer must carry with the exact value it must hold.
fn exact(expect: &Expect, answer: &Answer, faults: &mut Vec<String>) {
    let Some(wanted) = &expect.json else { return };
    for (key, value) in wanted {
        match held(answer, key) {
            Err(missing) => faults.push(missing.about(key)),
            Ok(found) if !same(value, found) => {
                faults.push(format!(
                    "{key} is {}, and it declares {}",
                    readable(found),
                    said(value)
                ));
            }
            Ok(_) => {}
        }
    }
}

/// The places an answer must carry, whatever they hold, and the kinds they must be.
fn keys(expect: &Expect, answer: &Answer, faults: &mut Vec<String>) {
    for key in expect.json_has_keys.iter().flatten() {
        if let Err(missing) = held(answer, key) {
            faults.push(missing.about(key));
        }
    }
    for (key, kind) in expect.json_types.iter().flatten() {
        match held(answer, key) {
            Err(missing) => faults.push(missing.about(key)),
            Ok(found) if !is_kind(found, *kind) => faults.push(format!(
                "{key} is {}, and it declares {}",
                kind_of(found),
                kind_said(*kind)
            )),
            Ok(_) => {}
        }
    }
    for (key, least) in expect.json_at_least.iter().flatten() {
        match held(answer, key) {
            Err(missing) => faults.push(missing.about(key)),
            Ok(found) => match found.as_i64() {
                None => faults.push(format!(
                    "{key} is {}, and it declares at least {least}",
                    readable(found)
                )),
                Some(found) if found < *least => {
                    faults.push(format!(
                        "{key} is {found}, and it declares at least {least}"
                    ));
                }
                Some(_) => {}
            },
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

/// Why an answer holds nothing at the place a key names.
///
/// Two, and the difference is whose fault it is. A key that names no place is a
/// manifest this build would refuse to read — reported here as well, because an
/// assertion nothing can evaluate must not come back as one that held. Everything else
/// is the answer, and what is worth saying about it is how far the way there got: *no
/// `MediaContainer`* and *`/MediaContainer/Setting` holds no entry whose `id` is that*
/// are the same verdict and different mornings.
enum Missing {
    /// The key names no place at all.
    Unreadable(String),
    /// The way there ran out, with what was reached before it did. Empty where it ran
    /// out at once, which is the flat case and wants no clause at all.
    Ran(String),
}

impl Missing {
    /// What to say about the key this happened to.
    fn about(&self, key: &str) -> String {
        match self {
            Self::Unreadable(why) => format!("{key} names no place in an answer: {why}"),
            Self::Ran(clause) if clause.is_empty() => format!("the body carries no {key}"),
            Self::Ran(clause) => format!("the body carries no {key}: {clause}"),
        }
    }
}

/// What the body holds at the place a key names.
///
/// A key is read by [`pointing`]: a plain name is a top-level member, which is what
/// every key written before this generation is, and one beginning with `/` is a JSON
/// Pointer that may pick an entry of a list by a field it holds.
fn held<'a>(answer: &'a Answer, key: &str) -> Result<&'a Value, Missing> {
    let steps = pointing::steps(key).map_err(|why| Missing::Unreadable(why.0))?;
    let Some(body) = answer.json.as_ref() else {
        return Err(Missing::Ran(
            "the body did not parse as a document".to_owned(),
        ));
    };

    let mut at = body;
    for (taken, step) in steps.iter().enumerate() {
        // Named only where the walk stops, because naming it is what a refusal wants
        // and every other step throws the string away. `get` rather than a slice: the
        // range comes from this loop's own index and cannot be out of bounds, and a
        // line that cannot panic beats a sentence saying why it could not.
        let reached = || place(steps.get(..taken).unwrap_or_default());
        at = match step {
            Step::Named(name) => match at.as_object() {
                None => return Err(Missing::Ran(format!("{} is {}", reached(), kind_of(at)))),
                Some(object) => object.get(name).ok_or_else(|| {
                    // Nothing was walked, so there is nowhere to name and the plain
                    // sentence says the whole of it.
                    Missing::Ran(if taken == 0 {
                        String::new()
                    } else {
                        format!("{} holds no {name}", reached())
                    })
                })?,
            },
            Step::Selected { field, value } => {
                let Some(entries) = at.as_array() else {
                    return Err(Missing::Ran(format!(
                        "{} is {}, and a selector picks an entry of a list",
                        reached(),
                        kind_of(at)
                    )));
                };
                let mut matched = entries
                    .iter()
                    .filter(|entry| entry.get(field).is_some_and(|held| is(held, value)));
                let first = matched.next().ok_or_else(|| {
                    Missing::Ran(format!(
                        "{} holds no entry whose {field} is {value:?}",
                        reached()
                    ))
                })?;
                let rest = matched.count();
                if rest > 0 {
                    return Err(Missing::Ran(format!(
                        "{} holds {} entries whose {field} is {value:?}, and a selector picks one",
                        reached(),
                        rest + 1
                    )));
                }
                first
            }
        };
    }
    Ok(at)
}

/// What to call the place a walk reached, where it reached one.
fn place(walked: &[Step]) -> String {
    if walked.is_empty() {
        return "the body".to_owned();
    }
    pointing::said(walked)
}

/// Whether an entry's field holds the value a selector names.
///
/// A selector's value is written as text, because a key is text. What it is compared
/// against is whatever the answer holds, so the three scalars each have their own
/// reading and nothing else matches: an object or a list is not a thing a key can spell,
/// and `null` is the absence a selector is looking past.
fn is(held: &Value, wanted: &str) -> bool {
    match held {
        Value::String(word) => word == wanted,
        Value::Number(number) => number.to_string() == wanted,
        Value::Bool(flag) => flag.to_string() == wanted,
        _ => false,
    }
}

/// A value as a refusal prints it, cut where it stops being readable.
fn readable(found: &Value) -> String {
    let whole = found.to_string();
    if whole.chars().count() <= READABLE {
        return whole;
    }
    let kept: String = whole.chars().take(READABLE).collect();
    format!("{kept}… ({} characters in all)", whole.chars().count())
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
        let read = serde_json::from_str::<Recording>(&text);
        assert!(read.is_ok(), "the recording does not read: {read:?}");
        read.map(|recording| recording.response).unwrap_or_default()
    }

    /// An expectation, read the way a manifest's is: through its own deserialiser.
    ///
    /// `Expect` refuses a field it does not know, so one mistyped key here would come
    /// back as an expectation saying nothing — against which every answer is faultless
    /// and a case asserting exactly that would pass having tested no rule at all.
    fn expects(declared: &str) -> Expect {
        let read = serde_json::from_str(declared);
        assert!(read.is_ok(), "the expectation does not read: {read:?}");
        read.unwrap_or_default()
    }

    #[test]
    fn an_answer_that_is_the_declared_one_has_nothing_wrong_with_it() {
        let faults = judge(
            &expects(r#"{"status": 401}"#),
            &answered(r#"{"status": 401, "headers": {"content-type": "application/json"}}"#),
        );
        assert_eq!(faults, Vec::<String>::new());
    }

    /// Every constraint a manifest can write, against an answer that meets it.
    ///
    /// Each rule below is otherwise only ever shown refusing. A rule that never passes
    /// is indistinguishable from a rule that always refuses, and the probe it decides
    /// would then be a proof no plugin could ever hold — three of these had no case
    /// demonstrating that the answer they accept exists.
    #[test]
    fn an_answer_meeting_every_constraint_it_declares_has_nothing_wrong_with_it() {
        let object = judge(
            &expects(
                r#"{"status": 200, "json": {"isClaimed": true, "count": 3, "kind": "x"},
                    "json_has_keys": ["isClaimed"], "json_types": {"count": "int"},
                    "json_at_least": {"count": 2}, "content_type": "application/json",
                    "body_starts_with": "{"}"#,
            ),
            &answered(
                r#"{"status": 200, "headers": {"content-type": "application/json"},
                    "body_starts_with": "{\"isClaimed\"",
                    "json": {"isClaimed": true, "count": 3, "kind": "x"}}"#,
            ),
        );
        assert_eq!(object, Vec::<String>::new());

        let list = judge(
            &expects(r#"{"json_array_min": 2}"#),
            &answered(r#"{"status": 200, "json": [1, 2, 3]}"#),
        );
        assert_eq!(list, Vec::<String>::new());

        let nothing = judge(
            &expects(r#"{"json_is_absent": true}"#),
            &answered(r#"{"status": 204}"#),
        );
        assert_eq!(nothing, Vec::<String>::new());
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

    /// A body that does not begin as declared says what it does begin with.
    ///
    /// The only fault of the nine that no case produced: the rule was demonstrated
    /// accepting a body and never refusing one, which is the same hole the other way
    /// round from the three above.
    #[test]
    fn a_body_that_begins_otherwise_says_what_it_begins_with() {
        let faults = judge(
            &expects(r#"{"body_starts_with": "<?xml"}"#),
            &answered(r#"{"status": 200, "body_starts_with": "<!DOCTYPE html>"}"#),
        );
        assert_eq!(
            faults,
            vec![r#"the body begins "<!DOCTYPE html>", and it declares "<?xml""#]
        );
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

    /// A place one level down, which is what most of the web answers with.
    ///
    /// The whole of what the flat vocabulary could say about this body was that
    /// `MediaContainer` was there, which is a probe passing because something replied.
    #[test]
    fn a_pointer_reaches_a_place_inside_the_answer() {
        let plex = answered(
            r#"{"status": 200, "json": {"MediaContainer": {"size": 0,
                "machineIdentifier": "1b9acc58", "claimed": false}}}"#,
        );
        let faults = judge(
            &expects(
                r#"{"status": 200,
                    "json": {"/MediaContainer/claimed": false},
                    "json_has_keys": ["/MediaContainer/machineIdentifier"],
                    "json_types": {"/MediaContainer/machineIdentifier": "str"},
                    "json_at_least": {"/MediaContainer/size": 0}}"#,
            ),
            &plex,
        );
        assert_eq!(faults, Vec::<String>::new());
    }

    /// The same place, holding something else, named as the place rather than as a key.
    #[test]
    fn a_place_holding_something_else_is_named_by_the_key_that_reached_it() {
        let faults = judge(
            &expects(r#"{"json": {"/MediaContainer/claimed": true}}"#),
            &answered(r#"{"status": 200, "json": {"MediaContainer": {"claimed": false}}}"#),
        );
        assert_eq!(
            faults,
            vec!["/MediaContainer/claimed is false, and it declares true"]
        );
    }

    /// A key that is not a pointer still means the top-level member of that name.
    ///
    /// The property every manifest written before pointers existed depends on, asked
    /// of a key with a dot in it because that is the one that would change meaning
    /// under a dotted-path reading.
    #[test]
    fn a_key_that_is_not_a_pointer_is_still_a_top_level_name() {
        let answer = answered(r#"{"status": 200, "json": {"MediaContainer.size": 3}}"#);
        assert_eq!(
            judge(&expects(r#"{"json": {"MediaContainer.size": 3}}"#), &answer),
            Vec::<String>::new()
        );
        // And reaches nothing where the body nests instead, rather than quietly
        // resolving a path somebody did not write.
        let nested = answered(r#"{"status": 200, "json": {"MediaContainer": {"size": 3}}}"#);
        assert_eq!(
            judge(
                &expects(r#"{"json_has_keys": ["MediaContainer.size"]}"#),
                &nested
            ),
            vec!["the body carries no MediaContainer.size"]
        );
    }

    /// The selector, against the answer it was written for.
    ///
    /// A hundred and fifty-one settings, found by the `id` one of them carries. The
    /// value is compared as text against whatever the entry holds, so a flag, a number
    /// and a word each have their own reading and each is asked for here.
    #[test]
    fn a_selector_picks_the_one_entry_a_field_names() {
        let prefs = answered(
            r#"{"status": 200, "json": {"MediaContainer": {"size": 3, "Setting": [
                {"id": "FriendlyName", "value": ""},
                {"id": "PublishServerOnPlexOnlineKey", "value": false},
                {"id": "TranscoderQuality", "value": 3}]}}}"#,
        );
        let faults = judge(
            &expects(
                r#"{"json": {
                    "/MediaContainer/Setting/[id=PublishServerOnPlexOnlineKey]/value": false,
                    "/MediaContainer/Setting/[id=TranscoderQuality]/value": 3,
                    "/MediaContainer/Setting/[id=FriendlyName]/value": ""}}"#,
            ),
            &prefs,
        );
        assert_eq!(faults, Vec::<String>::new());

        // Picking by a number and by a flag, which is the other half of the comparison.
        let byvalue = judge(
            &expects(r#"{"json": {"/MediaContainer/Setting/[value=3]/id": "TranscoderQuality"}}"#),
            &prefs,
        );
        assert_eq!(byvalue, Vec::<String>::new());
        let byflag = judge(
            &expects(
                r#"{"json": {"/MediaContainer/Setting/[value=false]/id":
                    "PublishServerOnPlexOnlineKey"}}"#,
            ),
            &prefs,
        );
        assert_eq!(byflag, Vec::<String>::new());

        // And the setting turned on, which is the whole reason the check exists.
        let published = answered(
            r#"{"status": 200, "json": {"MediaContainer": {"Setting": [
                {"id": "PublishServerOnPlexOnlineKey", "value": true}]}}}"#,
        );
        assert_eq!(
            judge(
                &expects(
                    r#"{"json": {
                        "/MediaContainer/Setting/[id=PublishServerOnPlexOnlineKey]/value": false}}"#
                ),
                &published
            ),
            vec![
                "/MediaContainer/Setting/[id=PublishServerOnPlexOnlineKey]/value is true, \
                 and it declares false"
            ]
        );
    }

    /// Every way the way there can run out, each saying where it stopped.
    ///
    /// A verdict of *the body carries no …* and nothing else is what a reader gets from
    /// a flat lookup, and against a nested answer it is the least useful true sentence
    /// available: it does not say whether the envelope was there.
    #[test]
    fn a_place_that_cannot_be_reached_says_how_far_it_got() {
        let body = answered(
            r#"{"status": 200, "json": {"MediaContainer": {"size": 0, "title": "Plex",
                "Setting": [{"id": "a", "value": 1}, {"id": "b", "value": 1}]}}}"#,
        );
        let asked = |key: &str| {
            judge(
                &expects(&format!(r#"{{"json_has_keys": ["{key}"]}}"#)),
                &body,
            )
        };
        assert_eq!(
            asked("/MediaContainer/nowhere"),
            vec!["the body carries no /MediaContainer/nowhere: /MediaContainer holds no nowhere"]
        );
        assert_eq!(
            asked("/MediaContainer/title/deeper"),
            vec![
                "the body carries no /MediaContainer/title/deeper: /MediaContainer/title is a \
                 string"
            ]
        );
        assert_eq!(
            asked("/MediaContainer/[id=a]"),
            vec![
                "the body carries no /MediaContainer/[id=a]: /MediaContainer is an object, and a \
                 selector picks an entry of a list"
            ]
        );
        assert_eq!(
            asked("/MediaContainer/Setting/[id=nobody]/value"),
            vec![
                "the body carries no /MediaContainer/Setting/[id=nobody]/value: \
                 /MediaContainer/Setting holds no entry whose id is \"nobody\""
            ]
        );
        assert_eq!(
            asked("/MediaContainer/Setting/[value=1]/id"),
            vec![
                "the body carries no /MediaContainer/Setting/[value=1]/id: \
                 /MediaContainer/Setting holds 2 entries whose value is \"1\", and a selector \
                 picks one"
            ]
        );
        // The first step, where there is nowhere to name and the plain sentence is the
        // whole of it — the wording every manifest written before pointers relies on.
        assert_eq!(asked("nowhere"), vec!["the body carries no nowhere"]);
        // The body itself being the wrong shape, which is a different sentence again.
        let list = answered(r#"{"status": 200, "json": [1, 2]}"#);
        assert_eq!(
            judge(&expects(r#"{"json_has_keys": ["content"]}"#), &list),
            vec!["the body carries no content: the body is a list"]
        );
        let nothing = answered(r#"{"status": 204}"#);
        assert_eq!(
            judge(&expects(r#"{"json_has_keys": ["content"]}"#), &nothing),
            vec!["the body carries no content: the body did not parse as a document"]
        );
    }

    /// A key that names no place is a fault rather than a place that is absent.
    ///
    /// The manifest reader refuses one before anything is run. This is the second
    /// answer to the same question, for the callers that did not come through it: an
    /// assertion nothing can evaluate must not read as one that held.
    #[test]
    fn a_key_naming_no_place_is_a_fault_that_says_so() {
        let faults = judge(
            &expects(r#"{"json_has_keys": ["/Setting[id=x]"]}"#),
            &answered(r#"{"status": 200, "json": {}}"#),
        );
        assert_eq!(faults.len(), 1, "got: {faults:?}");
        assert!(
            faults.first().is_some_and(
                |said| said.contains("names no place in an answer") && said.contains("bracket")
            ),
            "got: {faults:?}"
        );
    }

    /// A number asked of something that is not one says what is there.
    #[test]
    fn a_place_holding_no_number_cannot_answer_a_minimum() {
        let faults = judge(
            &expects(r#"{"json_at_least": {"/MediaContainer/title": 1}}"#),
            &answered(r#"{"status": 200, "json": {"MediaContainer": {"title": "Plex"}}}"#),
        );
        assert_eq!(
            faults,
            vec![r#"/MediaContainer/title is "Plex", and it declares at least 1"#]
        );
    }

    /// A refusal is read by somebody deciding which of two documents is wrong.
    ///
    /// Plex answers a hundred and fifty-one settings at one path, and the refusal that
    /// printed all of them said everything and showed nothing. Both sides of the cut
    /// are asked for, because a cap that always fired would be as useless as none.
    #[test]
    fn a_refusal_prints_no_more_of_a_value_than_can_be_read() {
        let short = judge(
            &expects(r#"{"json": {"a": "x"}}"#),
            &answered(r#"{"status": 200, "json": {"a": "short"}}"#),
        );
        assert_eq!(short, vec![r#"a is "short", and it declares "x""#]);

        let long: String = std::iter::repeat_n("settings", 60).collect();
        let faults = judge(
            &expects(r#"{"json": {"a": "x"}}"#),
            &answered(&format!(r#"{{"status": 200, "json": {{"a": "{long}"}}}}"#)),
        );
        let said = faults.first().cloned().unwrap_or_default();
        assert!(said.contains('…'), "it was cut: {said}");
        assert!(said.contains("characters in all"), "and says so: {said}");
        assert!(said.chars().count() < 200, "and is readable: {said}");
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
