//! A hand-edited configuration a quality sync answers, and what putting the profiles back
//! writes in its place, held to what may be shown before anything else reads them. The
//! adapter is not trusted and a crafted file is assumed, so masking fails closed: every
//! kept secret is taken out wherever it stands, and each file is read top to bottom, with
//! every parent key in view, line by line only where every line of it is one no YAML
//! reader can read another way: a plain key and a plain value, a list item, a comment or
//! nothing. In such a file nothing after a word naming a credential is shown, nor any line
//! beneath its key; any other file shows each line's key name at most. The diff shown is
//! the core's own, of the two masked files.

use crate::error::withheld::{is_secret, withheld_text, REDACTED};
use crate::materialised::diff;
use crate::model::StackEdit;
use crate::ports::service::Edited;

/// The longest file a quality sync may answer.
pub(super) const LARGEST: usize = 64 * 1024;

/// The longest path a hand edit may be named by.
const LONGEST_PATH: usize = 255;

/// What ends a word of a line besides whitespace: YAML's punctuation.
const BOUNDARIES: [char; 16] = [
    '{', '}', '[', ']', ',', ':', '=', '"', '\'', '?', '&', '*', '!', '#', '|', '>',
];

/// What may not stand in a plain value: YAML's indicators, quotes, escapes and tabs, any
/// of which a YAML reader may read apart from how the line reads, and the `@` and `%` a
/// login or an encoded name rides in an address on.
const UNPLAIN: [char; 17] = [
    '#', '&', '*', '!', '|', '>', '\'', '"', '{', '}', '[', ']', '`', '\\', '\t', '@', '%',
];

/// What may not open a plain value besides [`UNPLAIN`].
const UNOPENING: [char; 3] = ['-', '?', ':'];

/// Every value the secret store keeps, and every value each replaced; refused where there
/// is no store to read or it cannot be read, since a file cannot then be shown safely.
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

/// The diff of `edit`'s two files, each masked first, every one of `secrets` and every
/// credential it sets taken out; refused where either is longer than [`LARGEST`] or the
/// edit is named by something that is not a path.
pub(super) fn masked(edit: Edited, secrets: &[String]) -> Result<StackEdit, String> {
    let Edited {
        path,
        on_disk,
        preset,
    } = edit;
    if let Some(long) = [&on_disk, &preset]
        .into_iter()
        .find(|text| text.len() > LARGEST)
    {
        return Err(format!(
            "it answered a file of {} bytes, longer than the {LARGEST} it may",
            long.len()
        ));
    }
    if path.is_empty() || path.len() > LONGEST_PATH || path.chars().any(char::is_control) {
        return Err("it answered a hand edit under a name that is not a path".to_owned());
    }
    Ok(StackEdit {
        path: taken_out(&path, secrets),
        diff: diff(&file(&on_disk, secrets), &file(&preset, secrets)),
    })
}

/// `text`, a whole file, as it may be shown: every one of `secrets` taken out, then read
/// line by line top to bottom where every line is [`plain`], and by its keys alone
/// otherwise.
fn file(text: &str, secrets: &[String]) -> String {
    let text = taken_out(text, secrets);
    let line_by_line = text.lines().all(plain);
    let mut block = None;
    text.lines()
        .map(|line| {
            if line_by_line {
                keyed(line, &mut block)
            } else {
                unvouched(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `body` is a line no YAML reader reads another way than line by line: nothing, a
/// comment, a plain key alone or with a plain value, or a list item of a plain value or of
/// such a key.
fn plain(body: &str) -> bool {
    let rest = body.trim_start_matches(' ');
    if rest.is_empty() || rest.starts_with('#') {
        return !rest.contains('\t');
    }
    let (listed, rest) = rest
        .strip_prefix("- ")
        .map_or((false, rest), |item| (true, item));
    match rest.split_once(':') {
        Some((key, value)) => {
            plain_key(key) && (value.is_empty() || value.strip_prefix(' ').is_some_and(plain_value))
        }
        None => listed && plain_value(rest),
    }
}

/// Whether `key` is spelled only in letters, digits, `_`, `-` and `.`.
fn plain_key(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with('-')
        && key
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-' | '.'))
}

/// Whether `value` is a plain scalar on one line, read alike by every reader.
fn plain_value(value: &str) -> bool {
    let value = value.trim_matches(' ');
    !value.is_empty()
        && !value.starts_with(UNOPENING)
        && !value.contains(UNPLAIN)
        && !value.contains(": ")
        && !value.ends_with(':')
}

/// `body`, a [`plain`] line, with nothing after the first word naming a credential shown,
/// wholly masked where it stands beneath such a word's key, and with only its mark shown
/// where it is a comment; `block` holds the indentation of that word's line, and whether a
/// list at that indentation stands beneath it.
fn keyed(body: &str, block: &mut Option<(usize, bool)>) -> String {
    if body.trim().is_empty() {
        return body.to_owned();
    }
    let indent = indentation(body);
    if let Some((opened, listed)) = *block {
        let listing = listed && indent == opened && body.trim_start().starts_with('-');
        if indent > opened || listing {
            return format!("{}{REDACTED}", " ".repeat(indent));
        }
        *block = None;
    }
    if body.trim_start().starts_with('#') {
        return format!("{}# {REDACTED}", " ".repeat(indent));
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

/// `body`, a line of a file the core cannot read line by line, showing its key's name at
/// most.
fn unvouched(body: &str) -> String {
    if body.trim().is_empty() {
        return body.to_owned();
    }
    let indent = " ".repeat(indentation(body));
    let named = body
        .split_once([':', '='])
        .map(|(name, _)| name.trim())
        .filter(|name| plain_key(name));
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
