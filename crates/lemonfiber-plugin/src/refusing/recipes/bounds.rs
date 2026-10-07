//! What keeps a recipe a sequence: guards that only look back, retries within numbers
//! this format publishes, and captures that read a place an answer could hold.
//!
//! **A guard looks back and never jumps.** `when` names a step written before it, or a
//! value captured or brought in before it, so a recipe makes each step at most once, in
//! the order written, and always ends. A guard can skip a step and never add one, which
//! is why the flows a recipe could produce are every substitution in every step.
//!
//! **A wait is bounded by this format, not by the manifest.** How often a step is made
//! again, how long between two tries and how long a recipe waits in all are numbers the
//! format publishes, and one past them is refused when the manifest is read.

use std::collections::BTreeSet;

use crate::pointing;
use crate::schema::{Condition, Recipe, Step, ALL_WAITING, HEADER, LONGEST_WAIT, MOST_RETRIES};
use crate::Violation;

/// What one recipe has seen by the time it reaches a step: the steps before it and the
/// values brought in or captured before it.
pub(super) struct Before<'a> {
    /// The ids of the steps written before.
    pub(super) steps: BTreeSet<&'a str>,
    /// The names of the values available.
    pub(super) values: BTreeSet<&'a str>,
}

/// Refuse a guard that does not look back at something that came before it.
pub(super) fn guarded(step: &Step, before: &Before<'_>, at: &str, found: &mut Vec<Violation>) {
    let Some(when) = &step.when else {
        return;
    };
    let here = format!("{at}.when");
    match when {
        Condition {
            step: Some(earlier),
            status: Some(_),
            value: None,
            equals: None,
        } => {
            if !before.steps.contains(earlier.as_str()) {
                found.push(Violation {
                    location: here,
                    message: format!(
                        "asks about {earlier}, which is no step written before this one; a guard \
                         looks back, so a recipe makes each step at most once and always ends"
                    ),
                });
            }
        }
        Condition {
            step: None,
            status: None,
            value: Some(value),
            equals: Some(_),
        } => known(value, &before.values, &here, found),
        _ => found.push(shapeless(&here, "{ step, status } or { value, equals }")),
    }
}

/// Refuse a retry past the published bounds, or one whose end is not a status or a
/// value it could have captured.
pub(super) fn retried(step: &Step, before: &Before<'_>, at: &str, found: &mut Vec<Violation>) {
    let Some(retry) = &step.retry else {
        return;
    };
    let here = format!("{at}.retry");
    if retry.times == 0 || retry.times > MOST_RETRIES {
        found.push(Violation {
            location: format!("{here}.times"),
            message: format!(
                "{} is not between 1 and {MOST_RETRIES}, the most a step is made again",
                retry.times
            ),
        });
    }
    if retry.seconds().is_none() {
        found.push(Violation {
            location: format!("{here}.every"),
            message: format!(
                "{:?} is not a whole number of seconds from 1 to {LONGEST_WAIT}, written like \
                 `6s`",
                retry.every
            ),
        });
    }
    let mut seen: BTreeSet<&str> = before.values.clone();
    seen.extend(step.capture.iter().map(|capture| capture.name.as_str()));
    match &retry.until {
        Condition {
            step: None,
            status: Some(_),
            value: None,
            equals: None,
        } => {}
        Condition {
            step: None,
            status: None,
            value: Some(value),
            equals: Some(_),
        } => known(value, &seen, &format!("{here}.until"), found),
        _ => found.push(shapeless(
            &format!("{here}.until"),
            "{ status } or { value, equals }",
        )),
    }
}

/// Refuse a recipe whose retries could wait longer, in all, than the published bound.
pub(super) fn waiting(recipe: &Recipe, at: &str, found: &mut Vec<Violation>) {
    let all: u64 = recipe
        .steps
        .iter()
        .filter_map(|step| step.retry.as_ref())
        .filter_map(|retry| retry.seconds().map(|every| every * u64::from(retry.times)))
        .sum();
    if all > ALL_WAITING {
        found.push(Violation {
            location: at.to_owned(),
            message: format!(
                "could wait {all} seconds between retries, past the {ALL_WAITING} a recipe may \
                 wait in all"
            ),
        });
    }
}

/// Refuse a capture that reads no place an answer could hold.
pub(super) fn read(step: &Step, at: &str, found: &mut Vec<Violation>) {
    for capture in &step.capture {
        let place = match capture.from.strip_prefix(HEADER) {
            Some(header) if is_token(header) => Ok(()),
            Some(header) => Err(format!("{header:?} is not a header's name")),
            None => pointing::steps(&capture.from)
                .map(drop)
                .map_err(|why| why.to_string()),
        };
        if let Err(why) = place {
            found.push(Violation {
                location: format!("{at}.capture.from"),
                message: format!("{} names no place in an answer: {why}", capture.from),
            });
        }
    }
}

/// Whether text is a header's name: one or more of the characters HTTP allows in one.
fn is_token(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/// Refuse a guard on a value nothing before it captured or brought in.
fn known(value: &str, seen: &BTreeSet<&str>, at: &str, found: &mut Vec<Violation>) {
    if !seen.contains(value) {
        found.push(Violation {
            location: at.to_owned(),
            message: format!(
                "asks about {value}, which nothing before it captures or brings in, so whether \
                 it holds could not be read off the manifest"
            ),
        });
    }
}

/// Said of a condition carrying neither of its shapes, or parts of both.
fn shapeless(at: &str, shapes: &str) -> Violation {
    Violation {
        location: at.to_owned(),
        message: format!("is written as neither of its shapes, which are {shapes}"),
    }
}

#[cfg(test)]
mod tests;
