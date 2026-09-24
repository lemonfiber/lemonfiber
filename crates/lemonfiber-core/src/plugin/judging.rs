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

use std::collections::BTreeMap;

use lemonfiber_plugin::pointing::{self, Step};
use lemonfiber_plugin::{Expect, Expected, ExpectedKind};
use serde_json::Value;

use super::recorded::Answer;

/// The most of a value a refusal prints before it stops being readable.
///
/// A refusal is read by somebody holding a manifest and a recording and deciding which
/// of the two is wrong. Plex answers a hundred and fifty-one settings at `/:/prefs`, and
/// a refusal that printed all of them said everything and showed nothing.
const READABLE: usize = 120;

/// The method a declaration names, as the transport carries it.
///
/// Four, because four is what the port has. A declaration naming anything else cannot
/// be sent, and saying so is a better answer than sending a different method than the
/// one that was written down — a proof asked with the wrong verb is a proof about a
/// question nobody declared.
///
/// Beside the evaluator with the answer reader, because they are the two halves of one
/// seam: this is how a declared question reaches the transport, and that is how what
/// comes back reaches the rule. Two copies of either would be two ways of asking.
#[must_use]
pub(crate) fn method(declared: &str) -> Option<crate::ports::http::Method> {
    use crate::ports::http::Method;
    match declared.to_ascii_uppercase().as_str() {
        "GET" => Some(Method::Get),
        "POST" => Some(Method::Post),
        "PUT" => Some(Method::Put),
        "DELETE" => Some(Method::Delete),
        _ => None,
    }
}

/// A live answer, read into the shape a recorded one is read into.
///
/// Beside the evaluator rather than beside either caller, which is what lets one
/// evaluator serve both: a live answer and a recorded one are the same four facts, and
/// two evaluators for one vocabulary would be two things to keep in step.
///
/// Headers are folded to lower case and the first of a repeated one wins, matching how
/// the transport answers a question about one. A body is offered as a document where it
/// reads as one and as text either way, because an expectation may ask about either and
/// which it asks about is not this function's business.
pub(crate) fn live(response: &crate::ports::http::Response) -> Answer {
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in &response.headers {
        headers
            .entry(name.to_lowercase())
            .or_insert_with(|| value.clone());
    }
    Answer {
        status: response.status,
        headers,
        json: serde_json::from_str(&response.body).ok(),
        body_starts_with: Some(response.body.clone()),
    }
}

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
fn is_kind(found: &Value, kind: ExpectedKind) -> bool {
    match kind {
        ExpectedKind::Bool => found.is_boolean(),
        ExpectedKind::Int => found.is_i64() || found.is_u64(),
        ExpectedKind::Str => found.is_string(),
        ExpectedKind::List => found.is_array(),
        ExpectedKind::Dict => found.is_object(),
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
fn kind_said(kind: ExpectedKind) -> &'static str {
    match kind {
        ExpectedKind::Bool => "a true or a false",
        ExpectedKind::Int => "a number",
        ExpectedKind::Str => "a string",
        ExpectedKind::List => "a list",
        ExpectedKind::Dict => "an object",
    }
}

#[cfg(test)]
mod tests;
