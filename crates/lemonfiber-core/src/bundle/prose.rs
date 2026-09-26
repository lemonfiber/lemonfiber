//! Withholding a credential from text that names no fields.
//!
//! A service quoting its own configuration back in an error message is ordinary, so the
//! logs in a bundle carry credentials in running prose. This finds them by shape rather
//! than by name — sharing its tokenizer with the scan that reads the bundle back, so
//! whatever one writes the other accepts.

use super::{Filenames, Marks, Terms};

use super::scan::{key_shaped, reads_as_key};

/// The longest a run of an encoding's alphabet may be and still read as a word.
///
/// Base64 and its URL-safe cousin carry `+`, `/`, `=`, `-` and `_`, which the key
/// tokenizer splits on, so a `WireGuard` key read by the key rule is a handful of short
/// runs. Read whole, it is one long mixed run, and so is any encoded secret.
const ENCODED_LENGTH: usize = 24;

/// Free text as it may be shared: a log line, a finding, anything with no field names for
/// an allow-list to work from.
///
/// The allow-list cannot help here, so this is the narrower rule the same reasoning gives.
/// A query string goes wholesale — that is where the key nobody spotted actually lives,
/// riding inside something that looks like an address. Then any run long and dense enough
/// to read as a key is replaced whatever it sits next to, using the very tokeniser the
/// scan uses, so that what this writes is what the scan will accept.
///
/// Not the same rule as [`crate::config::store::withheld_text`], which faults already pass
/// through on their way to being remembered: that one reads names and withholds what
/// follows them, and this one reads values and knows no names at all. Two rules, two jobs.
/// What both leave is a key broken up by characters no key alphabet uses — several short
/// runs to the tokeniser, and to the scan as well.
///
/// Each line goes through [`crate::error::withheld::withheld`] first, which reads names
/// rather than values: a `password=` whose value is short and ordinary, which no shape
/// rule could tell from a word, is withheld because of what it is called.
#[must_use]
pub fn prose(text: &str, marks: &Marks, terms: &Terms) -> String {
    text.lines()
        .map(|line| said(&crate::error::withheld::withheld(line), marks, terms))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One line of free text, split on spaces rather than on whitespace so it comes back spaced
/// as it was written — a log line read by a person is half indentation.
fn said(line: &str, marks: &Marks, terms: &Terms) -> String {
    line.split(' ')
        .map(|word| spoken(word, marks, terms))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One word of free text.
fn spoken(word: &str, marks: &Marks, terms: &Terms) -> String {
    if terms.filenames == Filenames::Replaced && names_media(word) {
        return marks.of(word);
    }
    let word = without_userinfo(word, marks);
    match word.split_once('?') {
        // A question mark with parameters after it is a query string wherever it turns up;
        // one without is somebody asking a question in a log line. The address in front of
        // it still goes through the key rule, because a path can carry a key too, and
        // anything left whole here is something the scan would refuse the bundle over.
        Some((address, query)) if query.contains('=') => {
            format!("{}?{}", encoded(address, marks), marks.of(query))
        }
        _ => encoded(&word, marks),
    }
}

/// An address with whatever stands in front of its host marked.
///
/// The whole of the userinfo rather than the half after its colon: a service reached
/// as `https://<token>@host` authenticates by the name alone, and a rule that only knew
/// about passwords would print the token. Only the authority is read — an `@` in a path
/// or a query is somebody's address or a parameter, and the rules for those are below.
pub(super) fn without_userinfo(word: &str, marks: &Marks) -> String {
    let Some((scheme, rest)) = word.split_once("://") else {
        return word.to_owned();
    };
    let ends = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest.get(..ends).unwrap_or_default();
    let Some(at) = authority.rfind('@') else {
        return word.to_owned();
    };
    let userinfo = authority.get(..at).unwrap_or_default();
    let after = rest.get(at..).unwrap_or_default();
    format!("{scheme}://{}{after}", marks.of(userinfo))
}

/// Every run of an encoding's alphabet that reads as encoded, or carries an identifier
/// in the dashed form, replaced by its mark — and what is left read by the key rule.
fn encoded(word: &str, marks: &Marks) -> String {
    let mut spoken = String::new();
    let mut run = String::new();
    for character in word.chars() {
        if encoding_shaped(character) {
            run.push(character);
            continue;
        }
        spoken.push_str(&decided(&run, marks));
        run.clear();
        spoken.push(character);
    }
    spoken.push_str(&decided(&run, marks));
    spoken
}

/// One run of an encoding's alphabet: marked whole where it reads as encoded or holds a
/// dashed identifier, and otherwise handed to the key rule.
fn decided(run: &str, marks: &Marks) -> String {
    if reads_as_encoded(run) || holds_dashed_identifier(run) {
        return marks.of(run);
    }
    keys(run, marks)
}

/// Whether a character belongs to the base64 alphabet or its URL-safe variant.
fn encoding_shaped(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '+' | '/' | '=' | '-' | '_')
}

/// Whether a run reads as something encoded rather than as a path or a name.
///
/// Long, and carrying lower case, upper case and digits together, which a word and a
/// hexadecimal digest do not. A path is the one thing in a log line built from the same
/// alphabet at that length, and it starts with its separator and carries it often, so a
/// run that does either is read as a path.
fn reads_as_encoded(run: &str) -> bool {
    run.len() >= ENCODED_LENGTH
        && !run.starts_with('/')
        && run.matches('/').count() <= 3
        && run.chars().any(|character| character.is_ascii_lowercase())
        && run.chars().any(|character| character.is_ascii_uppercase())
        && run.chars().any(|character| character.is_ascii_digit())
}

/// Whether a run holds an identifier written as five dashed hexadecimal groups of
/// eight, four, four, four and twelve — the form a service hands out as a key or a
/// device's identity, and one whose groups are each too short for the key rule.
pub(super) fn holds_dashed_identifier(run: &str) -> bool {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let hex: Vec<&str> = run
        .split(|character: char| !(character.is_ascii_hexdigit() || character == '-'))
        .collect();
    hex.iter().any(|piece| {
        piece
            .split('-')
            .collect::<Vec<_>>()
            .windows(GROUPS.len())
            .any(|groups| {
                groups
                    .iter()
                    .zip(GROUPS)
                    .all(|(group, length)| group.len() == length)
            })
    })
}

/// Every key-shaped run in a word, replaced by its mark.
fn keys(word: &str, marks: &Marks) -> String {
    let mut spoken = String::new();
    let mut run = String::new();
    for character in word.chars() {
        if key_shaped(character) {
            run.push(character);
            continue;
        }
        spoken.push_str(&spelled(&run, marks));
        run.clear();
        spoken.push(character);
    }
    spoken.push_str(&spelled(&run, marks));
    spoken
}

/// One run, marked where it reads as a key and left alone where it does not.
fn spelled(run: &str, marks: &Marks) -> String {
    if reads_as_key(run) {
        return marks.of(run);
    }
    run.to_owned()
}

/// What a media file is called at the end.
///
/// The name rather than the path: what makes a filename worth replacing is the title in
/// it, and the title is in the name whatever directory it is sitting in.
const MEDIA: [&str; 8] = [
    ".mkv", ".mp4", ".avi", ".m4v", ".mov", ".srt", ".nfo", ".iso",
];

/// Whether a word names a media file.
fn names_media(word: &str) -> bool {
    let word = word.to_ascii_lowercase();
    MEDIA.iter().any(|extension| word.ends_with(extension))
}
