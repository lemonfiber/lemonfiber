//! A hand edit a quality sync answers, held to what may be shown before anything else
//! reads it. The adapter's own masking is not trusted, and a crafted edit is assumed: every
//! kept secret is taken out wherever it stands, each side of the diff is read as YAML and
//! every value under a key that names a credential is masked at any depth, and a side that
//! does not read as YAML has every value it sets masked.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde_norway::Value;

use crate::config::display::shown_in_a_file;
use crate::error::withheld::{withheld_by, withheld_text, REDACTED};
use crate::model::StackEdit;
use crate::ports::service::Edited;

/// The longest diff of a hand edit read from a quality sync.
pub(super) const LARGEST: usize = 64 * 1024;

/// The longest path a hand edit may be named by.
const LONGEST_PATH: usize = 255;

/// What a key's name holds, lower-cased and with `-`, `_`, `.` and spaces taken out, where
/// its value is a credential.
const CREDENTIAL_NAMES: [&str; 8] = [
    "apikey", "key", "token", "secret", "password", "passwd", "auth", "bearer",
];

/// Every value the secret store keeps, and every value each replaced.
pub(super) fn held(ctx: &crate::app::Ctx) -> Vec<String> {
    crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .map(|secrets| secrets.values().map(str::to_owned).collect())
        .unwrap_or_default()
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

    /// `lines` with every credential this side's lines set masked.
    fn masked(self, mut lines: Vec<Line>) -> Vec<Line> {
        let bodies: Vec<&str> = lines
            .iter()
            .filter(|line| line.side == self)
            .map(|line| line.body.as_str())
            .collect();
        let read = read(&bodies);
        let mut block: Option<usize> = None;
        for line in lines.iter_mut().filter(|line| line.side == self) {
            line.body = match &read {
                Some(values) => hidden(&line.body, values),
                None => unvouched(&line.body),
            };
            line.body = keyed(&line.body, &mut block);
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

/// Every scalar `bodies` set under a key naming a credential, read as YAML once their
/// shared indentation is taken off; nothing where they do not read as YAML.
fn read(bodies: &[&str]) -> Option<BTreeSet<String>> {
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
    let value: Value = serde_norway::from_str(&text.join("\n")).ok()?;
    let mut found = BTreeSet::new();
    gathered(&value, false, &mut found);
    Some(found)
}

/// Every scalar beneath `value` gathered into `found` where it stands under a key naming a
/// credential, or `beneath` one already.
fn gathered(value: &Value, beneath: bool, found: &mut BTreeSet<String>) {
    match value {
        Value::Mapping(mapping) => {
            for (key, value) in mapping {
                let named = key.as_str().is_some_and(names_a_credential);
                gathered(value, beneath || named, found);
            }
        }
        Value::Sequence(sequence) => {
            for value in sequence {
                gathered(value, beneath, found);
            }
        }
        Value::Tagged(tagged) => gathered(&tagged.value, beneath, found),
        Value::String(text) if beneath => {
            found.extend(
                text.lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned),
            );
        }
        Value::Number(number) if beneath => {
            found.insert(number.to_string());
        }
        Value::Bool(_) | Value::Null | Value::String(_) | Value::Number(_) => {}
    }
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

/// `body`, a line of a side that does not read as YAML, with any value it sets masked.
fn unvouched(body: &str) -> String {
    match split(body) {
        Some((name, separator, value)) if !value.trim().is_empty() => {
            format!("{name}{separator} {REDACTED}")
        }
        _ => body.to_owned(),
    }
}

/// `body` with the value masked where its key names a credential, and wholly masked where
/// it continues such a value from an earlier line; `block` holds the indentation of a key
/// whose value runs on below it.
fn keyed(body: &str, block: &mut Option<usize>) -> String {
    let indent = indentation(body);
    if let Some(opened) = *block {
        if body.trim().is_empty() || indent > opened {
            return format!("{}{REDACTED}", " ".repeat(indent));
        }
        *block = None;
    }
    let Some((name, separator, value)) = split(body) else {
        return body.to_owned();
    };
    if !names_a_credential(name) {
        return body.to_owned();
    }
    if runs_on(value) {
        *block = Some(indent);
    }
    format!("{name}{separator} {REDACTED}")
}

/// Whether a value opening on a key's line runs on to the lines below it: nothing yet, a
/// block scalar, or a quotation not closed on the line.
fn runs_on(value: &str) -> bool {
    let value = value.trim();
    value.is_empty()
        || value.starts_with(['|', '>'])
        || value.starts_with('&') && !value.contains(' ')
        || ['"', '\'']
            .iter()
            .any(|quote| value.starts_with(*quote) && value.matches(*quote).count() % 2 == 1)
}

/// `body` split at its first `:` or `=`: what stands before it, the separator, and what
/// follows.
fn split(body: &str) -> Option<(&str, char, &str)> {
    let (at, separator) = body
        .char_indices()
        .find(|(_, character)| matches!(character, ':' | '='))?;
    Some((
        body.get(..at)?,
        separator,
        body.get(at + separator.len_utf8()..)?,
    ))
}

/// Whether `name`, with its quotes, braces and list marker, names a credential.
fn names_a_credential(name: &str) -> bool {
    let normalised: String = name
        .chars()
        .filter(|character| !matches!(character, '-' | '_' | '.' | ' ' | '"' | '\'' | '{' | '['))
        .flat_map(char::to_lowercase)
        .collect();
    CREDENTIAL_NAMES
        .iter()
        .any(|credential| normalised.contains(credential))
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
