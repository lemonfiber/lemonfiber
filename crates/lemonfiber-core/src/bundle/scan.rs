//! Reading the bundle back, looking for anything that still resembles a credential.
//!
//! The check that makes the promise keepable: whatever the redactor missed, this finds
//! before an operator is told what the bundle holds. Its tokenizer is the redactor's, so
//! the two cannot disagree about what a key looks like.

use super::Terms;

/// Where a bundle was found to still hold something that reads as a credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Residual {
    /// The file it was found in, so a refusal can name what produced it.
    pub source: String,
    /// Which line of that file, counted from one as an operator would read it.
    pub line: usize,
}

/// The shortest run of unbroken key-shaped characters that reads as a credential.
///
/// Deliberately a different mechanism from the allow-list: two checks that fail the same
/// way are one check. This one knows nothing about names — it reads the values themselves,
/// and anything long enough and dense enough to be a key trips it whatever it is called.
const KEY_LENGTH: usize = 20;

/// The shortest run of an encoding's alphabet that reads as an encoded credential here.
///
/// Longer than the redactor's own threshold on purpose: this is the check behind the
/// redactor, so it trips only on what the redactor had every chance to mark and did
/// not, rather than refusing a bundle over a run the redactor judged a path.
const ENCODED_LENGTH: usize = 32;

/// How a value that has been withheld begins, in either of the two ways it can be.
const WITHHELD: [&str; 2] = ["<redacted:", "(set"];

/// Whether a bundle still holds anything that reads as a credential, and where.
///
/// Run over the assembled bundle rather than each piece as it is collected, because the
/// question is about what would actually be shared. A hit is not a warning: the bundle is
/// not written at all, and the file that produced it is named, because the one failure
/// this whole module exists to prevent is a bundle that looked fine and was not.
///
/// `terms` is read for one reason: a setting its operator named and confirmed is not a
/// residual. A scan that refused the bundle over the one value somebody deliberately put
/// in it would make the consent worth nothing.
#[must_use]
pub fn residual(files: &[(String, String)], terms: &Terms) -> Option<Residual> {
    for (source, body) in files {
        for (index, line) in body.lines().enumerate() {
            if chosen(line, terms) {
                continue;
            }
            // A line is read whole, marks and all. Skipping the ones that carry a mark
            // would let a line that holds both a mark and a leak pass on the strength of
            // the half that was handled — and a mark trips nothing here anyway: it is
            // four characters, and a key is twenty.
            if line.split(|c: char| !key_shaped(c)).any(reads_as_key)
                || encoded_in(line)
                || userinfo_in(line)
                || dashed_in(line)
                || named_in(line)
            {
                return Some(Residual {
                    source: source.clone(),
                    line: index + 1,
                });
            }
        }
    }
    None
}

/// Whether a line still holds a run of the base64 alphabet long and mixed enough to be
/// an encoded key.
///
/// Its own tokenizer rather than the redactor's: the redactor splits on `+`, `/` and
/// `=` to find keys, and a check that split the same way would pass a `WireGuard` key for
/// the same reason the redactor once did.
fn encoded_in(line: &str) -> bool {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_')))
        .any(|run| {
            run.len() >= ENCODED_LENGTH
                && !run.starts_with('/')
                && run.matches('/').count() <= 3
                && run.chars().any(|c| c.is_ascii_lowercase())
                && run.chars().any(|c| c.is_ascii_uppercase())
                && run.chars().any(|c| c.is_ascii_digit())
        })
}

/// Whether a line still holds an address with something in front of its host that has
/// not been withheld.
fn userinfo_in(line: &str) -> bool {
    line.match_indices("://").any(|(at, scheme)| {
        let rest = line.get(at + scheme.len()..).unwrap_or_default();
        let ends = rest
            .find(|c: char| matches!(c, '/' | '?' | '#') || c.is_whitespace())
            .unwrap_or(rest.len());
        let authority = rest.get(..ends).unwrap_or_default();
        authority.rfind('@').is_some_and(|at| {
            let userinfo = authority.get(..at).unwrap_or_default();
            !WITHHELD.iter().any(|mark| userinfo.starts_with(mark))
        })
    })
}

/// Whether a line still holds an identifier written as dashed hexadecimal groups of
/// eight, four, four, four and twelve.
fn dashed_in(line: &str) -> bool {
    line.split(|c: char| !(c.is_ascii_hexdigit() || c == '-'))
        .any(|piece| {
            let lengths: Vec<usize> = piece.split('-').map(str::len).collect();
            lengths.windows(5).any(|window| window == [8, 4, 4, 4, 12])
        })
}

/// Whether a line still holds a setting whose name says it is a credential, written
/// out as `name=value` or `name:value` with the value in the clear.
fn named_in(line: &str) -> bool {
    line.split_whitespace().any(|token| {
        let Some(at) = token.find(['=', ':']) else {
            return false;
        };
        let (name, value) = token.split_at(at);
        let value = value
            .get(1..)
            .unwrap_or_default()
            .trim_start_matches(['"', '\'']);
        let name = name.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_');
        !name.is_empty()
            && !value.is_empty()
            && !value.starts_with("//")
            && crate::error::withheld::is_secret(name)
            && !WITHHELD.iter().any(|mark| value.starts_with(mark))
    })
}

/// Whether this line is a setting the operator asked to have shown.
///
/// Matched as a setting rather than as a value, deliberately. What was consented to was
/// showing a *field*; the same key turning up in a log line was nobody's decision, and is
/// still a leak.
fn chosen(line: &str, terms: &Terms) -> bool {
    line.split_once('=')
        .is_some_and(|(name, _)| terms.reveals(name))
}

/// Whether a character could be part of a key: the alphabet every service in the stack
/// mints its own with, and nothing else.
pub(super) fn key_shaped(character: char) -> bool {
    character.is_ascii_alphanumeric()
}

/// Whether a run of characters reads as a credential rather than as a word.
///
/// Long, and mixed: prose is long too, but a word is letters, and a key of any length
/// anybody generates carries digits. Neither test alone is worth much and both together
/// have never yet called a sentence a secret.
pub(super) fn reads_as_key(run: &str) -> bool {
    run.len() >= KEY_LENGTH
        && run.chars().any(|character| character.is_ascii_digit())
        && run.chars().any(|character| character.is_ascii_alphabetic())
}
