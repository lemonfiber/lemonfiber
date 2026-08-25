//! What the screen asks a terminal to be able to do.
//!
//! The floor is a terminal that offers nothing: sixteen colours, a font with
//! letters in it, and not much room. Two of those three are guarded here. The
//! third is guarded where it is decided — the dashboard reflows below ninety-six
//! columns and is drawn at sixty by ninety and again at eight by four to prove it.
//!
//! **The boundary is the module trees that draw.** A file naming `ratatui` draws;
//! so does every file beside it under the same module, and those are checked too.
//! Drawing the line at the import instead would have taken `logs/draw.rs` and left
//! `logs/notices.rs`, which writes the lines `draw.rs` puts on the screen — one
//! half of a module held to a rule the other half is not.
//!
//! Outside it, `say.rs` and `render/` are the report surface. They write to a pipe
//! as readily as to a terminal, and what they may write is settled by the fold
//! those two already run every line through.
//!
//! **What is not covered**, since a guard that reads as wider than it is becomes a
//! reason not to look: these read the text this product writes. The glyphs ratatui
//! draws on its own account are not in any literal here, and a plain box border is
//! Unicode box-drawing whatever this file says.

use std::collections::{BTreeMap, BTreeSet};
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::str::Chars;

mod source_tree;

use source_tree::{production, sources};

/// The colours a terminal has before anything is negotiated.
///
/// The sixteen of the original ANSI set, plus the escape that gives a cell back to
/// whatever the terminal was already using. Everything past them is a request: a
/// palette index assumes a palette has been loaded, and a triple assumes the
/// terminal takes triples at all. Where it does not, the request is either dropped
/// or approximated, and neither is a thing the screen can be written against.
const PLAIN: &[&str] = &[
    "Reset",
    "Black",
    "Red",
    "Green",
    "Yellow",
    "Blue",
    "Magenta",
    "Cyan",
    "Gray",
    "DarkGray",
    "LightRed",
    "LightGreen",
    "LightYellow",
    "LightBlue",
    "LightMagenta",
    "LightCyan",
    "White",
];

/// The marks the screen writes that are not ASCII, and why each one is legible.
///
/// Both are older than every terminal this runs in and sit in the first Latin
/// block: a font that draws letters draws these. What the list refuses is the rest
/// — a glyph from a patched font, a private-use codepoint, a box-drawing run typed
/// out by hand — each of which is a bet on what somebody happened to install, and
/// each of which comes out as a blank box on the terminal that did not.
const LEGIBLE: &[(char, &str)] = &[
    (
        '\u{2014}',
        "an em dash, which the screen writes between a thing and what is wrong with it",
    ),
    (
        '\u{b7}',
        "a middle dot, which separates a rate from the reading beside it",
    ),
];

/// Every source file the screen is drawn from.
fn drawing() -> BTreeMap<PathBuf, String> {
    let all = sources();
    let mut trees: Vec<PathBuf> = Vec::new();
    for (path, text) in &all {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        if where_it_lives.contains("/src/") && production(text).contains("ratatui::") {
            trees.push(module_of(path));
        }
    }
    assert!(
        !trees.is_empty(),
        "nothing in this workspace draws, which means this is looking for the wrong name"
    );
    all.into_iter()
        .filter(|(path, _)| trees.iter().any(|tree| within(path, tree)))
        .collect()
}

/// The module tree one file belongs to.
///
/// A file directly under `src/` is a module on its own and stands for itself. One
/// any deeper shares a module with its siblings, and the tree is the directory
/// holding them.
fn module_of(path: &Path) -> PathBuf {
    match path.parent() {
        Some(parent) if parent.file_name().is_some_and(|name| name == "src") => path.to_path_buf(),
        Some(parent) => parent.to_path_buf(),
        None => path.to_path_buf(),
    }
}

/// Whether a file belongs to a module tree.
///
/// A directory tree takes the file that declares it as well as the files inside
/// it: `logs.rs` and `logs/` are one module, written in two places.
fn within(path: &Path, tree: &Path) -> bool {
    path.starts_with(tree) || path == tree.with_extension("rs")
}

/// The text inside double quotes on one line, which is what the screen is given
/// to draw.
///
/// Literals rather than whole lines: a mark this product must not draw is often a
/// perfectly ordinary thing to write in a comment, and a guard reading the file
/// would report the prose explaining the screen rather than the screen.
fn quoted(line: &str) -> impl Iterator<Item = &str> {
    line.split('"').skip(1).step_by(2)
}

/// The characters one literal puts on the screen.
///
/// Escapes are resolved rather than read as the backslash and letters they are
/// written with. A private-use codepoint spelled `\u{e0a0}` is plain ASCII in the
/// file and a glyph from somebody's patched font on the screen, and a guard reading
/// the bytes would have taken the file's word for it.
fn drawn(said: &str) -> Vec<char> {
    let mut marks = Vec::new();
    let mut written = said.chars().peekable();
    while let Some(mark) = written.next() {
        if mark != '\\' {
            marks.push(mark);
            continue;
        }
        match written.next() {
            Some('u') => marks.extend(escaped(&mut written)),
            Some(plain) => marks.push(plain),
            None => (),
        }
    }
    marks
}

/// The character a `\u{...}` escape stands for, the escape having been consumed.
fn escaped(written: &mut Peekable<Chars<'_>>) -> Option<char> {
    written.next_if(|mark| *mark == '{')?;
    let mut digits = String::new();
    while let Some(digit) = written.next_if(|mark| *mark != '}') {
        digits.push(digit);
    }
    written.next();
    u32::from_str_radix(&digits, 16)
        .ok()
        .and_then(char::from_u32)
}

/// Every colour a line asks for.
fn colours(line: &str) -> Vec<String> {
    line.match_indices("Color::")
        .filter_map(|(at, marker)| line.get(at + marker.len()..))
        .map(|rest| {
            rest.chars()
                .take_while(|letter| letter.is_alphanumeric() || *letter == '_')
                .collect()
        })
        .collect()
}

/// The screen asks for no colour a bare terminal does not already have.
///
/// A true-colour triple is not refused because it looks bad on a terminal without
/// it. It is refused because what happens there is not decided here: one terminal
/// approximates to the nearest of its own colours, another drops the escape and
/// leaves the text in whatever was in force, and a third prints the escape. Every
/// one of those is a screen nobody designed, and none of them is visible to
/// whoever wrote the triple on a machine where it worked.
#[test]
fn the_screen_asks_for_no_colour_a_bare_terminal_lacks() {
    let mut beyond: Vec<String> = Vec::new();
    for (path, text) in drawing() {
        for (number, line) in production(&text).lines().enumerate() {
            for named in colours(line) {
                if !PLAIN.contains(&named.as_str()) {
                    beyond.push(format!("{}:{}: `{named}`", path.display(), number + 1));
                }
            }
        }
    }
    assert!(
        beyond.is_empty(),
        "the screen asks for colours a terminal may not have, and what it does instead is \
         nobody's decision — name one of the sixteen: {beyond:?}"
    );
}

/// The screen writes nothing a plain font cannot draw.
///
/// Read from the text this product writes rather than from the frame it produces,
/// because a frame carries a release title and whatever a daemon said back, and
/// neither is ours to hold to a rule. The cost is that a mark reaching the screen
/// from a service's own words is not checked here; the gain is that this fails on
/// the line somebody typed rather than on the machine that happened to receive an
/// unusual title.
#[test]
fn the_screen_writes_nothing_a_plain_font_cannot_draw() {
    let legible: BTreeSet<char> = LEGIBLE.iter().map(|(mark, _)| *mark).collect();
    let mut beyond: Vec<String> = Vec::new();
    for (path, text) in drawing() {
        for (number, line) in production(&text).lines().enumerate() {
            for mark in quoted(line)
                .flat_map(drawn)
                .filter(|mark| !mark.is_ascii() && !legible.contains(mark))
            {
                beyond.push(format!(
                    "{}:{}: `{mark}` (U+{:04X})",
                    path.display(),
                    number + 1,
                    u32::from(mark)
                ));
            }
        }
    }
    assert!(
        beyond.is_empty(),
        "the screen writes these and a terminal font may have no glyph for them — write each \
         in ASCII, or add it to LEGIBLE with the reason a plain font draws it: {beyond:?}"
    );
}

/// Every mark declared legible says why it is.
///
/// The list is where a judgement about somebody else's font gets made. A blank
/// entry makes it without saying anything, which is the same as not making it.
#[test]
fn every_legible_mark_says_why_a_plain_font_draws_it() {
    let silent: Vec<char> = LEGIBLE
        .iter()
        .filter(|(_, reason)| reason.split_whitespace().count() < 4)
        .map(|(mark, _)| *mark)
        .collect();
    assert!(
        silent.is_empty(),
        "these are drawn on a font nobody here chose and the list does not say why that is \
         safe: {silent:?}"
    );
}
