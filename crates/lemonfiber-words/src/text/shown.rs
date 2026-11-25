//! Text somebody else wrote, as an answer carries it: what would lay it out or move a
//! cursor written as its bytes in hex, and no longer than the answer has room for.

/// The characters that lay text out rather than being shown, first and last of each run:
/// Unicode's format characters, the line and paragraph separators and the direction
/// overrides and isolates among them.
const LAYING_OUT: [(char, char); 21] = [
    ('\u{ad}', '\u{ad}'),
    ('\u{600}', '\u{605}'),
    ('\u{61c}', '\u{61c}'),
    ('\u{6dd}', '\u{6dd}'),
    ('\u{70f}', '\u{70f}'),
    ('\u{890}', '\u{891}'),
    ('\u{8e2}', '\u{8e2}'),
    ('\u{180e}', '\u{180e}'),
    ('\u{200b}', '\u{200f}'),
    ('\u{2028}', '\u{202e}'),
    ('\u{2060}', '\u{2064}'),
    ('\u{2066}', '\u{206f}'),
    ('\u{feff}', '\u{feff}'),
    ('\u{fff9}', '\u{fffb}'),
    ('\u{110bd}', '\u{110bd}'),
    ('\u{110cd}', '\u{110cd}'),
    ('\u{13430}', '\u{1343f}'),
    ('\u{1bca0}', '\u{1bca3}'),
    ('\u{1d173}', '\u{1d17a}'),
    ('\u{e0001}', '\u{e0001}'),
    ('\u{e0020}', '\u{e007f}'),
];

/// A text as it is shown, and whether it is the start of a longer one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bounded {
    /// The text, at most as long as it was bounded to.
    pub text: String,
    /// Whether the text went on past it.
    pub cut: bool,
}

/// `text` a character at a time, each as it is shown.
#[must_use]
pub fn shown(text: &str) -> Vec<String> {
    text.chars()
        .map(|letter| hidden(letter).unwrap_or_else(|| letter.to_string()))
        .collect()
}

/// `letter` as its UTF-8 in hex, each byte `\xx`, where it is a control character or one
/// that lays text out rather than being shown; nothing otherwise.
#[must_use]
pub fn hidden(letter: char) -> Option<String> {
    let lays_out = LAYING_OUT
        .iter()
        .any(|(first, last)| (*first..=*last).contains(&letter));
    (letter.is_control() || lays_out).then(|| {
        let mut bytes = [0; 4];
        letter
            .encode_utf8(&mut bytes)
            .bytes()
            .map(|byte| format!("\\{byte:02x}"))
            .collect::<Vec<_>>()
            .concat()
    })
}

/// `units` written out, up to `longest` characters, and whether there were more.
///
/// Cut between units, so a character written as its bytes is never cut in half.
#[must_use]
pub fn bounded(units: &[String], longest: usize) -> Bounded {
    let mut text = String::new();
    let mut width = 0;
    for unit in units {
        width += unit.chars().count();
        if width > longest {
            return Bounded { text, cut: true };
        }
        text.push_str(unit);
    }
    Bounded { text, cut: false }
}
