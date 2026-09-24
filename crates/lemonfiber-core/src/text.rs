//! Text from somewhere else, made safe to put on a terminal.
//!
//! Most of what this product shows an operator did not come from this product. A
//! release name comes from an indexer, a failure message comes from a \*arr, a
//! container name comes from an image somebody else built. All of it is written
//! straight to a terminal, and a terminal is not a text box: a control character
//! in the middle of a release name is an instruction to the emulator.
//!
//! `\x1b[2J` clears the screen. `\x1b[H` moves the cursor home. A carriage return
//! writes over the line just printed. None of these run code, and none of them are
//! exotic — they are what happens when a release name contains something it should
//! not, and the result is a screen that no longer says what this product said.
//!
//! So text from anywhere else passes through here on its way to being shown. What
//! is stripped is only what a terminal reads as an instruction; everything a
//! person could have meant — accents, scripts, punctuation, emoji — survives
//! untouched, because a release name in Japanese is a release name.
//!
//! That is the answer where a person is reading. Where a parser is, the same text
//! is [`escaped`] rather than made plain: a script asked for a value it could act
//! on, and a name with a character taken out of it no longer matches what the
//! service holds — while a `\uXXXX` in its place is the same string to anything
//! that reads JSON and six harmless characters to a terminal.
//!
//! Beside them live the two other things that happen to text on its way to being
//! read: breaking it so it fits the room there is, and — where the room is one row
//! and cannot be given a second — shortening it to that row. The two surfaces that
//! wrap want different things at the edge, and [`Overrun`] is how each says which.

/// The same text with anything a terminal would obey removed.
///
/// Removed rather than replaced with a marker: a marker would put a character
/// where the operator's copy of the name has none, and this text is commonly
/// matched against what a service holds. Losing a byte an emulator would have
/// swallowed anyway loses nothing.
#[must_use]
pub fn plain(text: &str) -> String {
    text.chars()
        .filter(|character| !obeyed(*character))
        .collect()
}

/// Whether a terminal would read this character as an instruction rather than as
/// something to draw.
///
/// The C0 range and delete, the C1 range that some emulators still act on, and the
/// Unicode separators that move a cursor without being control characters. Not
/// `is_control` alone: that misses C1 written as a single code point, which is the
/// form that survives a UTF-8 round trip through a service's JSON.
///
/// Then the two families that instruct without being control characters at all.
/// The bidirectional embeddings, overrides and isolates say which way the text
/// after them runs, so `\u{202e}` in a release name draws `gpj.exe` as `exe.jpg`
/// — the name on the screen is not the name in the queue, and the operator is
/// reading the attacker's version of it. The zero-width space and the byte-order
/// mark draw nothing at all, which is how two names that differ by one of them
/// read as the same name.
///
/// The marks that are *needed* to draw somebody's language are not here. A
/// zero-width joiner holds an emoji sequence together and separates a Persian
/// word's forms; the left-to-right and right-to-left marks settle which way a
/// neutral character leans in mixed text. None of those reverses a run — that
/// takes an override — and dropping them would misspell the name rather than
/// disarm it.
const fn obeyed(character: char) -> bool {
    matches!(
        character,
        '\u{0}'..='\u{1f}'
            | '\u{7f}'..='\u{9f}'
            | '\u{200b}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
    )
}

/// The three a JSON document may be laid out with.
///
/// A terminal obeys all three, and inside a string every one of them arrives
/// written out — but between two tokens they are whitespace, and writing one out
/// there produces an escape where a document expected a space. Serialising here is
/// compact, so today there are none; a pretty-printed document later would carry
/// them, and would be corrupted rather than disarmed.
const fn laid_out(character: char) -> bool {
    matches!(character, '\t' | '\n' | '\r')
}

/// The same document with anything a terminal would obey written as an escape.
///
/// The parser's door, where making text plain is the wrong answer: a script asked
/// for something it could parse, and taking a character out of a release name
/// changes the value it is handed. An escape changes nothing — a character and the
/// `\uXXXX` standing for it are the same string to every reader of JSON — and a
/// terminal the document is printed to reads six ASCII characters instead of an
/// instruction.
///
/// It was skipped here on the grounds that serialising had already made the text
/// safe, and that is true of exactly the range below a space. Above it, `serde_json`
/// carries the character raw: the C1 controls some emulators still act on, the two
/// line separators, the bidirectional overrides that draw a name backwards, and the
/// zero-widths that draw nothing at all. So `--json` was the one way out that a
/// release title could still reach a terminal through intact.
///
/// The set is [`obeyed`]'s, so the two doors cannot come to disagree about what a
/// terminal obeys — a character added there is answered on both.
#[must_use]
pub fn escaped(document: &str) -> String {
    let mut written = String::with_capacity(document.len());
    for character in document.chars() {
        if obeyed(character) && !laid_out(character) {
            written.push_str("\\u");
            for shift in [12_u32, 8, 4, 0] {
                written.push(digit((u32::from(character) >> shift) & 0xf));
            }
        } else {
            written.push(character);
        }
    }
    written
}

/// One hex digit, lower case, which is how JSON writes an escape.
///
/// Taken a digit at a time rather than formatted, because a `format!` into the
/// string it is building is an allocation a lint refuses and a `write!` is a result
/// nothing here can act on — a string cannot fail to be written to. The nibble is
/// masked to four bits before it arrives, so the fallback stands for nothing that
/// can happen and is there because a digit has to be returned.
fn digit(nibble: u32) -> char {
    char::from_digit(nibble, 16).unwrap_or('0')
}

/// What a run with nothing to break on does when it reaches the edge.
///
/// A report's width is a preference: it is read at whatever width the reader's
/// terminal is, which re-wraps an overrun. A screen's is a wall: it is a grid of
/// cells, and past the edge is not re-wrapped, it is not drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overrun {
    /// Runs past the edge, keeping the run whole.
    Allowed,
    /// Broken at the edge, the alternative being that its tail is never seen.
    Broken,
}

/// The text broken so that no line is longer than this width.
///
/// Broken at whitespace wherever there is whitespace to break at, the space a break
/// is taken at being spent on the break. Spaces inside a line are left as they
/// arrived: a service that aligned its own output with them meant them.
///
/// A run with nothing to break on — a path, a URL, a hash — is broken at the edge or
/// left whole according to what the caller says its edge is.
#[must_use]
pub fn wrapped(text: &str, width: usize, overrun: Overrun) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let marks: Vec<char> = text.chars().collect();
    let mut lines: Vec<String> = Vec::new();
    let mut at = 0;
    while at < marks.len() {
        let end = broken_at(&marks, at, width, overrun);
        let line: String = marks.get(at..end).unwrap_or_default().iter().collect();
        lines.push(line.trim_end().to_owned());
        at = end;
        while marks.get(at).is_some_and(|mark| mark.is_whitespace()) {
            at += 1;
        }
    }
    lines
}

/// Where the line starting here ends.
///
/// The character at the edge is looked at as well as the ones before it, so text
/// that fills the width exactly and is followed by a space ends where it fills
/// rather than one word short of it.
///
/// A break is only taken at whitespace that has something in front of it. Taking one
/// at the whitespace a line opens with would end a line that had not started, and
/// the caller would read the same run forever — so indentation a service wrote
/// stays with the line it indents.
fn broken_at(marks: &[char], at: usize, width: usize, overrun: Overrun) -> usize {
    let edge = at.saturating_add(width).min(marks.len());
    if edge == marks.len() {
        return edge;
    }
    let window = marks.get(at..=edge).unwrap_or_default();
    let opens = window.iter().position(|mark| !mark.is_whitespace());
    match window.iter().rposition(|mark| mark.is_whitespace()) {
        Some(space) if opens.is_some_and(|first| space > first) => at + space,
        _ if matches!(overrun, Overrun::Allowed) => marks
            .iter()
            .skip(edge)
            .position(|mark| mark.is_whitespace())
            .map_or(marks.len(), |past| edge + past),
        _ => edge,
    }
}

/// What stands in a value where the middle of it was left out.
///
/// Three full stops rather than an ellipsis, so a terminal that cannot render the
/// character is never handed one.
const MARKER: &str = "...";

/// The text shortened to this width, keeping both of its ends.
///
/// Elided in the middle rather than cut at the end, because the end of a value is
/// where the things that tell two of them apart live — for a release name, the
/// resolution, the encoding and the group. Cut at the tail, `...1080p` and
/// `...2160p` read identically, and a list that cannot tell two of its entries
/// apart fails at the one question it exists to answer.
///
/// For a row that cannot be given a second row this is what wrapping is instead:
/// a panel of a fixed height that wrapped would push its last entries out of the
/// box, which trades a loss that is marked for one that is silent.
///
/// Where there is not room for the marker and something of both ends, the marker
/// stands alone: a half of a name is read as a name, and a marker is not.
#[must_use]
pub fn fitted(text: &str, width: usize) -> String {
    let counted = text.chars().count();
    if counted <= width {
        return text.to_owned();
    }
    if width < MARKER.len() {
        return MARKER.chars().take(width).collect();
    }
    let keep = width - MARKER.len();
    let tail = keep / 2;
    let head = keep - tail;
    let front: String = text.chars().take(head).collect();
    let back: String = text.chars().skip(counted - tail).collect();
    format!("{front}{MARKER}{back}")
}

#[cfg(test)]
mod tests;
