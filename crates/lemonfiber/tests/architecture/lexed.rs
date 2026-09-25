//! Every comment in a Rust file, told from the code around it by lexing.
//!
//! A scan for `//` is wrong in Rust in both directions. `let s = "// not a
//! comment";` is code the scan indicts, and `let x = 1; // narration` is a comment
//! the scan cannot see at all, because deciding what a comment is by whether a
//! trimmed line opens with two slashes only ever finds the ones that stand alone.
//! The second is the worse of the two: a rule that silently ignores a whole comment
//! position reads as a tree that has been checked.
//!
//! So the reader is a lexer — rustc's own, published as `ra-ap-rustc_lexer` — and
//! it reports comments rather than code, which is the half the rules below are about.
//!
//! **The rules are values, not assertions.** Each returns what it found, so the same
//! rule runs over the tree that must be clean and over a tree of planted violations
//! where it must find its own and nothing else.

use std::path::Path;

/// One comment, as the lexer found it.
pub(crate) struct Comment {
    /// The line it opens on, counting from one.
    ///
    /// Where it closes is not carried, because no rule asks: the length rules count
    /// runs of `//`, each of which is one line, and the rules that read a comment's
    /// words read all of them whichever lines they fall on.
    pub(crate) line: usize,
    /// Whether rustdoc reads it: `///`, `//!`, `/**` or `/*!`.
    pub(crate) doc: bool,
    /// Whether it is a `/* … */` rather than a `//`.
    pub(crate) block: bool,
    /// Whether anything but whitespace stands before it on its opening line.
    pub(crate) trailing: bool,
    /// The whole of it, markers included, as written.
    pub(crate) said: String,
}

impl Comment {
    /// Whether it is an informative `//` on a line of its own.
    ///
    /// The three qualifications are what a block under the length rules is made of:
    /// a doc comment is rustdoc and answers to a different rule, a block comment is
    /// refused outright, and one written after code is attached to that code rather
    /// than standing with the lines around it.
    pub(crate) const fn standing(&self) -> bool {
        !self.doc && !self.block && !self.trailing
    }
}

/// One thing a rule found, where it found it.
pub(crate) struct Violation {
    /// The line it is on.
    pub(crate) line: usize,
    /// What is wrong with it, said as the author would need to read it.
    pub(crate) said: String,
}

/// Every comment in one Rust file, in the order they open.
///
/// Read with rustc's own lexer, so what counts as a comment is decided the way the
/// compiler decides it rather than by a second lexer kept here.
pub(crate) fn comments(text: &str) -> Vec<Comment> {
    let mut found = Vec::new();
    let mut at = 0;
    let mut line = 1;
    let mut code_on_line = false;
    for token in rustc_lexer::tokenize(text, rustc_lexer::FrontmatterAllowed::No) {
        let length = token.len as usize;
        let said = text.get(at..at + length).unwrap_or_default();
        match token.kind {
            rustc_lexer::TokenKind::LineComment { doc_style } => found.push(Comment {
                line,
                doc: doc_style.is_some(),
                block: false,
                trailing: code_on_line,
                said: said.to_owned(),
            }),
            rustc_lexer::TokenKind::BlockComment { doc_style, .. } => found.push(Comment {
                line,
                doc: doc_style.is_some(),
                block: true,
                trailing: code_on_line,
                said: said.to_owned(),
            }),
            rustc_lexer::TokenKind::Whitespace => {}
            _ => code_on_line = true,
        }
        let breaks = said.matches('\n').count();
        if breaks > 0 {
            line += breaks;
            code_on_line = false;
        }
        at += length;
    }
    found
}

/// How many lines of `//` a run must hold, and how many it may.
///
/// The floor is a relevance filter: a note that cannot be expanded into two lines of
/// genuine reason is a note restating what the next line does, and the code should
/// say it instead. The ceiling is the other end of the same judgement — past it the
/// prose is architecture, and architecture is a page in `.docs/` that can be revised.
const FLOOR: usize = 2;

/// The most lines of `//` one run may hold.
const CEILING: usize = 4;

/// The words that say a thing is unfinished.
///
/// Read as whole words. `\uXXXX` is how this workspace writes a JSON escape, and a
/// rule that read it as a deferral marker would be three false accusations in files
/// that are about exactly that notation.
const DEFERRALS: &[&str] = &["TODO", "FIXME", "HACK", "XXX"];

/// The identifier prefixes a comment may not carry.
const PREFIXES: &[&str] = &["ARCH-R", "REPO-R", "GOV-R", "OPS-R", "Q-R", "DES-R", "ADR-"];

/// A note written after code.
///
/// The shape a scan for a line opening in two slashes cannot see at all, which is
/// how a whole comment position came to be unchecked. It can never be part of a
/// block, so whatever reason it carries has one line to carry it in — and one line
/// is where narration lives: the value restated, the call named again.
pub(crate) fn note_after_code(text: &str, _root: &Path) -> Vec<Violation> {
    comments(text)
        .into_iter()
        .filter(|one| one.trailing && !one.doc && !one.block)
        .map(|one| Violation {
            line: one.line,
            said: "a note written after code, which can never be part of a block — put \
                   it above the line, or say it in the code"
                .to_owned(),
        })
        .collect()
}

/// A note on a line of its own, and nothing standing with it.
pub(crate) fn lone_comment(text: &str, _root: &Path) -> Vec<Violation> {
    runs(text)
        .into_iter()
        .filter(|(_, held)| *held < FLOOR)
        .map(|(line, _)| Violation {
            line,
            said: "a note on its own — say it in the code, or give the reason its second \
                   line"
                .to_owned(),
        })
        .collect()
}

/// A run of `//` longer than prose belongs in.
pub(crate) fn overlong_block(text: &str, _root: &Path) -> Vec<Violation> {
    runs(text)
        .into_iter()
        .filter(|(_, held)| *held > CEILING)
        .map(|(line, held)| Violation {
            line,
            said: format!(
                "{held} lines of prose — past {CEILING} it is a page in `.docs/` with \
                 a link to it from here"
            ),
        })
        .collect()
}

/// A `/* … */` that is not a doc comment.
///
/// The form has no use an informative `//` block does not cover, and it is the one
/// that can hide an arbitrary amount of text on a line that looks like code.
pub(crate) fn block_comment(text: &str, _root: &Path) -> Vec<Violation> {
    comments(text)
        .into_iter()
        .filter(|one| one.block && !one.doc)
        .map(|one| Violation {
            line: one.line,
            said: "a block comment — write it as a `//` block instead".to_owned(),
        })
        .collect()
}

/// A word saying the work is not finished.
pub(crate) fn deferral_token(text: &str, _root: &Path) -> Vec<Violation> {
    let mut found = Vec::new();
    for one in comments(text) {
        for token in DEFERRALS {
            if !holds(&one.said, token) {
                continue;
            }
            found.push(Violation {
                line: one.line,
                said: format!("says `{token}` — shipped code is finished, so finish it or cut it"),
            });
        }
    }
    found
}

/// An identifier standing in for the reason a line exists.
///
/// Provenance is not documentation. A comment naming which planning artefact caused
/// a line is worthless to the next reader and rots the moment that artefact is
/// superseded; the reason belongs in `.docs/`, reached from here by a link.
pub(crate) fn requirement_identifier(text: &str, _root: &Path) -> Vec<Violation> {
    let mut found = Vec::new();
    for one in comments(text) {
        for prefix in PREFIXES {
            if one.said.contains(prefix) {
                found.push(Violation {
                    line: one.line,
                    said: format!("cites `{prefix}…` — cite it in the commit instead"),
                });
            }
        }
        if cites_a_feature_requirement(&one.said) {
            found.push(Violation {
                line: one.line,
                said: "cites a requirement — cite it in the commit instead".to_owned(),
            });
        }
    }
    found
}

/// A page under `.docs/` named by a comment that is not there.
///
/// Every form the workspace names one in: a backticked path, a Markdown link, and
/// the absolute address of the same file on the forge. The path is read from
/// `.docs/` to the extension, so which of the three it was written as does not
/// change what is looked for.
pub(crate) fn unresolved_doc_link(text: &str, root: &Path) -> Vec<Violation> {
    let mut found = Vec::new();
    for one in comments(text) {
        for named in pages(&one.said) {
            if !root.join(&named).is_file() {
                found.push(Violation {
                    line: one.line,
                    said: format!("names `{named}`, which is not a page in this repository"),
                });
            }
        }
    }
    found
}

/// Every `.docs/…` page one comment names.
fn pages(said: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = said;
    while let Some(at) = rest.find(".docs/") {
        let after = rest.get(at..).unwrap_or_default();
        if let Some(end) = after.find(".md") {
            found.extend(after.get(..end + 3).map(str::to_owned));
        }
        rest = rest.get(at + 6..).unwrap_or_default();
    }
    found
}

/// One rule: what it is given, and what it found.
pub(crate) type Rule = fn(&str, &Path) -> Vec<Violation>;

/// Every rule, by the name a report calls it.
pub(crate) const RULES: &[(&str, Rule)] = &[
    ("a note written after code", note_after_code),
    ("a note standing alone", lone_comment),
    ("a run longer than prose belongs in", overlong_block),
    ("a block comment", block_comment),
    ("a deferral token", deferral_token),
    ("a requirement identifier", requirement_identifier),
    ("a page that is not there", unresolved_doc_link),
];

/// Every run of contiguous `//` lines standing on their own, and how long each is.
fn runs(text: &str) -> Vec<(usize, usize)> {
    let standing: Vec<usize> = comments(text)
        .into_iter()
        .filter(Comment::standing)
        .map(|one| one.line)
        .collect();
    let mut found: Vec<(usize, usize)> = Vec::new();
    for line in standing {
        match found.last_mut() {
            Some((first, held)) if *first + *held == line => *held += 1,
            _ => found.push((line, 1)),
        }
    }
    found
}

/// Whether a character can stand inside a name.
fn named(letter: char) -> bool {
    letter.is_alphanumeric() || letter == '_'
}

/// Whether a word stands in this text as a word rather than inside another.
fn holds(said: &str, word: &str) -> bool {
    let letters: Vec<char> = said.chars().collect();
    let wanted: Vec<char> = word.chars().collect();
    letters.windows(wanted.len()).enumerate().any(|(at, run)| {
        run == wanted.as_slice()
            && !letters.get(at.wrapping_sub(1)).copied().is_some_and(named)
            && !letters.get(at + wanted.len()).copied().is_some_and(named)
    })
}

/// Whether a comment writes a feature-area requirement identifier.
///
/// A scan rather than a window of fixed width. A window of five characters can only
/// ever see a feature number of one digit, and the specification defines four areas
/// whose number is two — every identifier in those was invisible to a rule that
/// names itself after catching them.
fn cites_a_feature_requirement(said: &str) -> bool {
    let letters: Vec<char> = said.chars().collect();
    for (at, letter) in letters.iter().enumerate() {
        if !letter.is_ascii_uppercase() {
            continue;
        }
        let mut after = at + 1;
        while letters.get(after).is_some_and(char::is_ascii_digit) {
            after += 1;
        }
        if after == at + 1 {
            continue;
        }
        let reads = [
            letters.get(after),
            letters.get(after + 1),
            letters.get(after + 2),
        ];
        if let [Some('-'), Some('R'), Some(index)] = reads {
            if index.is_ascii_digit() {
                return true;
            }
        }
    }
    false
}
