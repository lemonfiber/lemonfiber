//! A hand edit a quality sync answers, held to what may be shown before anything else
//! reads it. The adapter's own masking is not trusted and a crafted edit is assumed, so
//! masking fails closed: every kept secret is taken out wherever it stands, every line
//! naming a credential's key shows nothing after that key, and a side of the diff the
//! core cannot account for line by line shows no value it sets at all.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde_norway::Value;

use crate::config::display::shown_in_a_file;
use crate::error::withheld::{is_secret, withheld_by, withheld_text, REDACTED};
use crate::model::StackEdit;
use crate::ports::service::Edited;

/// The longest diff of a hand edit read from a quality sync.
pub(super) const LARGEST: usize = 64 * 1024;

/// The longest path a hand edit may be named by.
const LONGEST_PATH: usize = 255;

/// What ends a word of a line besides whitespace: YAML's punctuation.
const BOUNDARIES: [char; 16] = [
    '{', '}', '[', ']', ',', ':', '=', '"', '\'', '?', '&', '*', '!', '#', '|', '>',
];

/// Every value the secret store keeps, and every value each replaced; refused where there
/// is no store to read or it cannot be read, since an edit cannot then be shown safely.
pub(super) fn held(ctx: &crate::app::Ctx) -> Result<Vec<String>, String> {
    let at = crate::settings::secrets_at(ctx)
        .ok_or_else(|| "there is no secret store to hold the edit against".to_owned())?;
    let secrets = crate::settings::Secrets::at(&at)
        .map_err(|_| "the secret store could not be read to hold the edit against".to_owned())?;
    Ok(secrets.values().map(str::to_owned).collect())
}

/// What a sync said in refusing, as it may be shown: withheld as prose is, with every one
/// of `secrets` taken out.
pub(super) fn said(refused: &str, secrets: &[String]) -> String {
    withheld_text(&taken_out(refused, secrets))
}

/// `edit` as it may be shown, every one of `secrets` and every credential it sets masked;
/// refused where it is longer than [`LARGEST`] or named by something that is not a path.
pub(super) fn masked(edit: Edited, secrets: &[String]) -> Result<StackEdit, String> {
    let Edited { path, diff } = edit;
    if diff.len() > LARGEST {
        return Err(format!(
            "it answered a hand edit of {} bytes, longer than the {LARGEST} it may",
            diff.len()
        ));
    }
    if path.is_empty() || path.len() > LONGEST_PATH || path.chars().any(char::is_control) {
        return Err("it answered a hand edit under a name that is not a path".to_owned());
    }
    let lines: Vec<Line> = diff
        .lines()
        .map(|line| Line::of(&taken_out(line, secrets)))
        .collect();
    let diff = Side::ALL
        .iter()
        .fold(lines, |lines, side| side.masked(lines))
        .into_iter()
        .fold(String::new(), |mut diff, line| {
            let _ = writeln!(
                diff,
                "{}{}",
                line.marker,
                withheld_by(&line.body, &shown_in_a_file)
            );
            diff
        });
    Ok(StackEdit {
        path: taken_out(&path, secrets),
        diff,
    })
}

/// Which side of the diff a line stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// Only in what is on disk.
    Removed,
    /// Only in what is written in its place.
    Added,
    /// In both.
    Kept,
}

impl Side {
    /// Every side.
    const ALL: [Self; 3] = [Self::Removed, Self::Added, Self::Kept];

    /// `lines` with this side's lines masked: line by line where every credential the
    /// side sets is accounted for on a line that names its key, and every value hidden
    /// otherwise.
    fn masked(self, mut lines: Vec<Line>) -> Vec<Line> {
        let bodies: Vec<String> = lines
            .iter()
            .filter(|line| line.side == self)
            .map(|line| line.body.clone())
            .collect();
        let mut block = None;
        let named: usize = bodies.iter().map(|body| keys_named(body)).sum();
        let keyed: Vec<String> = bodies.iter().map(|body| keyed(body, &mut block)).collect();
        let shown: Vec<String> = match accounted(&bodies, named) {
            Some(values) => keyed.iter().map(|body| hidden(body, &values)).collect(),
            None => bodies.iter().map(|body| unvouched(body)).collect(),
        };
        for (line, body) in lines.iter_mut().filter(|line| line.side == self).zip(shown) {
            line.body = body;
        }
        lines
    }
}

/// One line of a diff: its marker, its side, and what follows the marker.
#[derive(Debug)]
struct Line {
    /// The `-` or `+` it opens with and the space after it, or nothing.
    marker: String,
    /// Its side.
    side: Side,
    /// What follows the marker.
    body: String,
}

impl Line {
    /// `line`, read as a line of a diff.
    fn of(line: &str) -> Self {
        let side = match line.chars().next() {
            Some('-') => Side::Removed,
            Some('+') => Side::Added,
            _ => Side::Kept,
        };
        let after = match side {
            Side::Kept => 0,
            Side::Removed | Side::Added => 1 + usize::from(line.get(1..2) == Some(" ")),
        };
        Self {
            marker: line.get(..after).unwrap_or_default().to_owned(),
            side,
            body: line.get(after..).unwrap_or_default().to_owned(),
        }
    }
}

/// Every scalar `bodies` set under a key naming a credential, where they read as one YAML
/// mapping with no anchor, alias, explicit key or collection running over lines, and name
/// no more such keys than `named` counts on lines of their own; nothing otherwise.
fn accounted(bodies: &[String], named: usize) -> Option<BTreeSet<String>> {
    if bodies.iter().any(|body| unaccountable(body)) {
        return None;
    }
    let indent = bodies
        .iter()
        .filter(|body| !body.trim().is_empty())
        .map(|body| indentation(body))
        .min()
        .unwrap_or_default();
    let text: Vec<&str> = bodies
        .iter()
        .map(|body| {
            body.get(indent.min(indentation(body))..)
                .unwrap_or_default()
        })
        .collect();
    if text.iter().all(|line| line.trim().is_empty()) {
        return Some(BTreeSet::new());
    }
    let value: Value = serde_norway::from_str(&text.join("\n")).ok()?;
    if !matches!(value, Value::Mapping(_)) {
        return None;
    }
    let mut found = BTreeSet::new();
    (credentials(&value, false, &mut found) <= named).then_some(found)
}

/// Whether `body` holds what the core does not place line by line: an anchor or alias, an
/// explicit key, or a flow collection left open at its end.
fn unaccountable(body: &str) -> bool {
    let opened = body.matches(['{', '[']).count();
    let closed = body.matches(['}', ']']).count();
    body.trim_start().starts_with('?')
        || opened != closed
        || words(body).any(|(start, _)| {
            body.get(..start)
                .and_then(|ahead| ahead.chars().next_back())
                .is_some_and(|mark| matches!(mark, '&' | '*'))
        })
}

/// How many keys naming a credential stand beneath `value`, every scalar beneath one
/// gathered into `found`, `beneath` where `value` already stands under one.
fn credentials(value: &Value, beneath: bool, found: &mut BTreeSet<String>) -> usize {
    match value {
        Value::Mapping(mapping) => mapping
            .iter()
            .map(|(key, value)| {
                let named = key.as_str().is_some_and(names_a_credential);
                usize::from(named) + credentials(value, beneath || named, found)
            })
            .sum(),
        Value::Sequence(sequence) => sequence
            .iter()
            .map(|value| credentials(value, beneath, found))
            .sum(),
        Value::Tagged(tagged) => credentials(&tagged.value, beneath, found),
        Value::String(text) if beneath => {
            found.extend(
                text.lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned),
            );
            0
        }
        Value::Number(number) if beneath => {
            found.insert(number.to_string());
            0
        }
        Value::Bool(_) | Value::Null | Value::String(_) | Value::Number(_) => 0,
    }
}

/// How many words of `body` name a credential and stand as a key, a separator after them.
fn keys_named(body: &str) -> usize {
    words(body)
        .filter(|(start, end)| {
            body.get(*start..*end).is_some_and(names_a_credential)
                && body
                    .get(*end..)
                    .map(|rest| rest.trim_start_matches(['"', '\'', ' ']))
                    .is_some_and(|rest| rest.starts_with([':', '=']))
        })
        .count()
}

/// `body` with nothing after the first word naming a credential shown, and wholly masked
/// where it continues such a word's value from an earlier line; `block` holds the
/// indentation of that word's line, and whether a list at that indentation continues it.
fn keyed(body: &str, block: &mut Option<(usize, bool)>) -> String {
    let indent = indentation(body);
    if let Some((opened, listed)) = *block {
        let listing = listed && indent == opened && body.trim_start().starts_with('-');
        if body.trim().is_empty() || indent > opened || listing {
            return format!("{}{REDACTED}", " ".repeat(indent));
        }
        *block = None;
    }
    let Some((_, end)) =
        words(body).find(|(start, end)| body.get(*start..*end).is_some_and(names_a_credential))
    else {
        return body.to_owned();
    };
    let rest = body.get(end..).unwrap_or_default();
    let value = rest.trim_start_matches(['"', '\'', ' ', ':', '=']);
    *block = Some((indent, value.trim().is_empty()));
    let shown = body.get(..body.len() - value.len()).unwrap_or_default();
    format!("{} {REDACTED}", shown.trim_end())
}

/// `body` with every one of `values` masked where it stands as a scalar of its own.
fn hidden(body: &str, values: &BTreeSet<String>) -> String {
    values
        .iter()
        .fold(body.to_owned(), |body, value| bounded(&body, value))
}

/// `body` with every occurrence of `value` not run into a longer word replaced.
fn bounded(body: &str, value: &str) -> String {
    let word = |character: Option<char>| character.is_some_and(char::is_alphanumeric);
    let mut out = String::new();
    let mut rest = body;
    while let Some(at) = rest.find(value) {
        let before = rest
            .get(..at)
            .and_then(|ahead| ahead.chars().next_back())
            .or_else(|| out.chars().next_back());
        let after = rest
            .get(at + value.len()..)
            .and_then(|tail| tail.chars().next());
        out.push_str(rest.get(..at).unwrap_or_default());
        out.push_str(if word(before) || word(after) {
            value
        } else {
            REDACTED
        });
        rest = rest.get(at + value.len()..).unwrap_or_default();
    }
    out.push_str(rest);
    out
}

/// `body`, a line of a side the core cannot account for, showing its key's name at most.
fn unvouched(body: &str) -> String {
    if body.trim().is_empty() {
        return body.to_owned();
    }
    let indent = " ".repeat(indentation(body));
    let named = body
        .split_once([':', '='])
        .map(|(name, _)| name.trim())
        .filter(|name| {
            !name.is_empty()
                && name.chars().all(|character| {
                    character.is_alphanumeric() || matches!(character, '_' | '-' | '.')
                })
        });
    match named {
        Some(name) => format!("{indent}{name}: {REDACTED}"),
        None => format!("{indent}{REDACTED}"),
    }
}

/// Where each word of `body` starts and ends, words being split at whitespace and at
/// [`BOUNDARIES`].
fn words(body: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut start = None;
    body.char_indices()
        .chain(std::iter::once((body.len(), ' ')))
        .filter_map(move |(at, character)| {
            let ends = character.is_whitespace() || BOUNDARIES.contains(&character);
            match (start, ends) {
                (None, false) => {
                    start = Some(at);
                    None
                }
                (Some(from), true) => {
                    start = None;
                    Some((from, at))
                }
                _ => None,
            }
        })
}

/// Whether `name`, with its quotes, braces and list marker and every `-`, `_`, `.` and
/// space taken out, names a credential as a setting's name does.
fn names_a_credential(name: &str) -> bool {
    let normalised: String = name
        .chars()
        .filter(|character| !matches!(character, '-' | '_' | '.' | ' ' | '"' | '\'' | '{' | '['))
        .collect();
    is_secret(&normalised)
}

/// How many spaces `body` opens with.
fn indentation(body: &str) -> usize {
    body.len() - body.trim_start_matches(' ').len()
}

/// `text` with every one of `secrets`, as it stands and percent-encoded, replaced.
fn taken_out(text: &str, secrets: &[String]) -> String {
    secrets
        .iter()
        .filter(|secret| !secret.is_empty())
        .flat_map(|secret| {
            [
                secret.clone(),
                encoded(secret, false),
                encoded(secret, true),
            ]
        })
        .fold(text.to_owned(), |text, secret| {
            text.replace(secret.as_str(), REDACTED)
        })
}

/// `secret` percent-encoded, every byte but the unreserved ones written as `%XX`, in
/// lower-case hexadecimal where `lower`.
fn encoded(secret: &str, lower: bool) -> String {
    secret
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            _ if lower => format!("%{byte:02x}"),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests;
