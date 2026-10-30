//! A hand edit a quality sync answers, held to what may be shown before anything else
//! reads it. The adapter's own masking is not trusted and a crafted edit is assumed, so
//! masking fails closed. Every kept secret is taken out wherever it stands. Each version
//! of the file the diff holds, what was on disk and what is written in its place, is read
//! in file order, and line by line only where every line of it is one no YAML reader can
//! read another way: a plain key and a plain value, a list item, a comment or nothing. In
//! such a version nothing after a word naming a credential is shown, nor any line
//! continuing its value, nor any line whose parent the version does not show; any other
//! version shows each line's key name at most. A line stands as the more masked of the
//! versions it belongs to.

use std::fmt::Write as _;

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

/// What may not stand in a plain value: YAML's indicators, quotes, escapes and tabs, any
/// of which a YAML reader may read apart from how the line reads, and the `@` and `%` a
/// login or an encoded name rides in an address on.
const UNPLAIN: [char; 17] = [
    '#', '&', '*', '!', '|', '>', '\'', '"', '{', '}', '[', ']', '`', '\\', '\t', '@', '%',
];

/// What may not open a plain value besides [`UNPLAIN`].
const UNOPENING: [char; 3] = ['-', '?', ':'];

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
    let old = Version::Old.shown(&lines);
    let new = Version::New.shown(&lines);
    let diff = lines.iter().zip(old.into_iter().zip(new)).fold(
        String::new(),
        |mut diff, (line, shown)| {
            let body = match (line.side, shown) {
                (Side::Hunk, _) => header(&line.body),
                (_, (Some(old), Some(new))) if old != new => unvouched(&line.body),
                (_, (Some(shown), _) | (None, Some(shown))) => shown,
                (_, (None, None)) => unvouched(&line.body),
            };
            let _ = writeln!(
                diff,
                "{}{}",
                line.marker,
                withheld_by(&body, &shown_in_a_file)
            );
            diff
        },
    );
    Ok(StackEdit {
        path: taken_out(&path, secrets),
        diff,
    })
}

/// Which side of the diff a line stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// Only in what was on disk.
    Removed,
    /// Only in what is written in its place.
    Added,
    /// In both.
    Kept,
    /// A hunk's header, where the lines after it stop following on from those before.
    Hunk,
}

/// One version of the file the diff holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Version {
    /// What was on disk: the lines kept and the lines removed.
    Old,
    /// What is written in its place: the lines kept and the lines added.
    New,
}

impl Version {
    /// Whether this version holds a line on `side`.
    const fn holds(self, side: Side) -> bool {
        matches!(
            (self, side),
            (_, Side::Kept) | (Self::Old, Side::Removed) | (Self::New, Side::Added)
        )
    }

    /// What this version shows of each of `lines`, walked in file order, or nothing for a
    /// line it does not hold: line by line where every line it holds is [`plain`], and
    /// with no value shown otherwise.
    fn shown(self, lines: &[Line]) -> Vec<Option<String>> {
        let plain = lines
            .iter()
            .filter(|line| self.holds(line.side))
            .all(|line| plain(&line.body));
        let mut block = None;
        let mut above: Vec<Above> = Vec::new();
        lines
            .iter()
            .map(|line| {
                if line.side == Side::Hunk {
                    block = None;
                    above.clear();
                }
                if !self.holds(line.side) {
                    return None;
                }
                let orphaned = orphan(&line.body, &above);
                if let Some(one) = Above::of(&line.body) {
                    above.push(one);
                }
                let keyed = keyed(&line.body, &mut block);
                Some(if !plain || orphaned {
                    unvouched(&line.body)
                } else {
                    keyed
                })
            })
            .collect()
    }
}

/// A line a version has shown since its hunk began, as a later line finds its parent.
#[derive(Debug, Clone, Copy)]
struct Above {
    /// Its indentation.
    indent: usize,
    /// Whether it is a key whose value is what follows below it.
    opens: bool,
}

impl Above {
    /// `body` as a later line finds its parent; nothing where it is blank.
    fn of(body: &str) -> Option<Self> {
        let rest = body.trim_start_matches(' ');
        (!rest.is_empty()).then(|| Self {
            indent: indentation(body),
            opens: rest.trim_end().ends_with(':'),
        })
    }
}

/// Whether `body` is indented, or a list item, with no parent among `above`: no line less
/// indented, and for a list item no key opening at its own indentation either.
fn orphan(body: &str, above: &[Above]) -> bool {
    let rest = body.trim_start_matches(' ');
    let indent = indentation(body);
    let listed = rest.starts_with('-');
    if rest.is_empty() || rest.starts_with('#') || (indent == 0 && !listed) {
        return false;
    }
    !above
        .iter()
        .rev()
        .any(|one| one.indent < indent || (listed && one.indent == indent && one.opens))
}

/// A hunk's header as it may be shown: its line numbers, and nothing of what follows them.
fn header(line: &str) -> String {
    let numbers = line
        .strip_prefix("@@")
        .and_then(|rest| rest.split_once("@@"))
        .map(|(numbers, _)| numbers)
        .filter(|numbers| {
            numbers.chars().all(|character| {
                character.is_ascii_digit() || matches!(character, ' ' | '-' | '+' | ',')
            })
        });
    numbers.map_or_else(
        || format!("@@ {REDACTED}"),
        |numbers| format!("@@{numbers}@@"),
    )
}

/// One line of a diff: its marker, its side, and what follows the marker.
#[derive(Debug)]
struct Line {
    /// The `-` or `+` it opens with and the space after it, the space a kept line opens
    /// with, or nothing.
    marker: String,
    /// Its side.
    side: Side,
    /// What follows the marker.
    body: String,
}

impl Line {
    /// `line`, read as a line of a diff.
    fn of(line: &str) -> Self {
        let side = if line.starts_with("@@") {
            Side::Hunk
        } else {
            match line.chars().next() {
                Some('-') => Side::Removed,
                Some('+') => Side::Added,
                _ => Side::Kept,
            }
        };
        let after = match side {
            Side::Hunk => 0,
            Side::Kept => usize::from(line.starts_with(' ')),
            Side::Removed | Side::Added => 1 + usize::from(line.get(1..2) == Some(" ")),
        };
        Self {
            marker: line.get(..after).unwrap_or_default().to_owned(),
            side,
            body: line.get(after..).unwrap_or_default().to_owned(),
        }
    }
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
/// wholly masked where it continues such a word's value from an earlier line, and with
/// only its mark shown where it is a comment; `block` holds the indentation of that word's
/// line, and whether a list at that indentation continues it.
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
