//! The address as something to point a camera at.
//!
//! Reading an address off a screen and typing it into a phone is where the handing
//! over goes wrong — a household member mistypes it once and asks the operator to
//! come and look, which is the interruption this product exists to remove.
//!
//! **The ink is the light modules, not the dark ones.** A terminal draws text in its
//! foreground colour, which on the dark terminal an operator is almost certainly
//! sitting at is the light one. Drawing the dark modules would hand a camera the
//! code inverted. The quiet zone is painted in the same ink rather than left as
//! background, so the code carries its own margin instead of borrowing whatever the
//! terminal happens to be — a border of unlit cells is not a quiet zone if the
//! surrounding screen is unlit too.
//!
//! This is right on a dark terminal and inverted on a light one. There is no way to
//! be right on both without setting a colour, and this surface writes to a pipe as
//! readily as to a screen, so it sets none.

use qrcodegen::{QrCode, QrCodeEcc};

/// How many modules of margin a reader needs around the code.
///
/// Four is what the specification asks for. Less and a camera can fail to find the
/// edge against whatever is printed beside it.
const QUIET: i32 = 4;

/// The address drawn as a code, or nothing if it will not fit in one.
///
/// `ascii` picks the drawing rather than being looked up here, so both can be tested
/// without settling a value that would outlive the test — the same division
/// `say::shown` makes for the fold.
pub(super) fn rows(address: &str, ascii: bool) -> Option<Vec<String>> {
    // Medium correction: a code read off a screen is not creased or faded, so the
    // budget is better spent on staying small than on surviving damage.
    let code = QrCode::encode_text(address, QrCodeEcc::Medium).ok()?;
    Some(if ascii { wide(&code) } else { stacked(&code) })
}

/// Whether the reader should see a dark module here.
///
/// Everything outside the code is light, which is what makes the margin a margin.
fn dark(code: &QrCode, x: i32, y: i32) -> bool {
    code.get_module(x - QUIET, y - QUIET)
}

/// The width and height of the code with its margin, in modules.
fn across(code: &QrCode) -> i32 {
    code.size() + QUIET * 2
}

/// Two module rows to a line, using the half-height blocks.
///
/// A terminal cell is about twice as tall as it is wide, so a module drawn as one
/// cell comes out stretched and a camera has more to correct for. Two rows in one
/// cell puts it back to square and halves the height, which is what lets the whole
/// code sit on a screen beside the text it repeats.
fn stacked(code: &QrCode) -> Vec<String> {
    let span = across(code);
    (0..span)
        .step_by(2)
        .map(|y| {
            (0..span)
                .map(|x| {
                    let over = !dark(code, x, y);
                    // The last line of an odd-height code has no lower row. Light,
                    // so it reads as the margin continuing rather than as a module.
                    let under = y + 1 >= span || !dark(code, x, y + 1);
                    match (over, under) {
                        (true, true) => '\u{2588}',
                        (true, false) => '\u{2580}',
                        (false, true) => '\u{2584}',
                        (false, false) => ' ',
                    }
                })
                .collect()
        })
        .collect()
}

/// One module row to a line, two characters wide, for a terminal without the blocks.
///
/// Twice the stacked drawing in both directions, so the code stays square and only
/// its size changes. It is the larger picture of the two, which is the trade a
/// terminal that cannot draw a half block leaves: a code that takes more screen
/// still reads, and no code at all does not.
fn wide(code: &QrCode) -> Vec<String> {
    let span = across(code);
    (0..span)
        .map(|y| {
            (0..span)
                .flat_map(|x| (if dark(code, x, y) { "  " } else { "##" }).chars())
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests;
