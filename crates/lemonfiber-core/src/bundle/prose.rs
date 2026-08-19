//! Withholding a credential from text that names no fields.
//!
//! A service quoting its own configuration back in an error message is ordinary, so the
//! logs in a bundle carry credentials in running prose. This finds them by shape rather
//! than by name — sharing its tokenizer with the scan that reads the bundle back, so
//! whatever one writes the other accepts.

use super::{Filenames, Marks, Terms};

use super::scan::{key_shaped, reads_as_key};

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
#[must_use]
pub fn prose(text: &str, marks: &Marks, terms: &Terms) -> String {
    text.lines()
        .map(|line| said(line, marks, terms))
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
    match word.split_once('?') {
        // A question mark with parameters after it is a query string wherever it turns up;
        // one without is somebody asking a question in a log line. The address in front of
        // it still goes through the key rule, because a path can carry a key too, and
        // anything left whole here is something the scan would refuse the bundle over.
        Some((address, query)) if query.contains('=') => {
            format!("{}?{}", keys(address, marks), marks.of(query))
        }
        _ => keys(word, marks),
    }
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
