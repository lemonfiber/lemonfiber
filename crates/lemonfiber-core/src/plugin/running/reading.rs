//! What a step takes out of its answer, and whether what came before holds.
//!
//! A capture reads a place an expectation could look at, or a header by its name; a
//! condition asks what an earlier step answered or what an earlier value holds. Both are
//! answered from what this run has seen and nothing else.

use std::collections::BTreeMap;

use lemonfiber_plugin::{Capture, Condition, HEADER};

use crate::plugin::Answer;

/// Every value these captures take out of one answer, or why one of them could not be.
pub(super) fn captured(
    captures: &[Capture],
    answer: &Answer,
) -> Result<BTreeMap<String, String>, String> {
    captures
        .iter()
        .map(|capture| {
            let value = match capture.from.strip_prefix(HEADER) {
                Some(header) => answer
                    .headers
                    .get(&header.to_ascii_lowercase())
                    .cloned()
                    .ok_or_else(|| format!("the answer carries no {header} header")),
                None => crate::plugin::judging::taken(answer, &capture.from),
            };
            value.map(|value| (capture.name.clone(), value))
        })
        .collect()
}

/// Whether a condition holds against what this run has seen: the status each step made
/// so far answered, and every value it holds.
///
/// A step that was skipped, or that nothing answered, answered no status, so a guard on
/// it does not hold. A condition in neither shape was refused when the manifest was
/// read, and holds nothing here.
pub(super) fn holds(
    condition: &Condition,
    answered: &BTreeMap<String, u16>,
    values: &BTreeMap<String, String>,
) -> bool {
    match condition {
        Condition {
            step: Some(step),
            status: Some(status),
            ..
        } => answered.get(step) == Some(status),
        Condition {
            value: Some(value),
            equals: Some(equals),
            ..
        } => values.get(value) == Some(equals),
        _ => false,
    }
}

/// Whether a retry's end holds for the answer just read: the status it answered, or a
/// value it captured or the run already held.
pub(super) fn ended(until: &Condition, status: u16, values: &BTreeMap<String, String>) -> bool {
    match until {
        Condition {
            status: Some(wanted),
            step: None,
            ..
        } => status == *wanted,
        _ => holds(until, &BTreeMap::new(), values),
    }
}

#[cfg(test)]
mod tests;
