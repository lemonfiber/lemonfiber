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
mod tests {
    use super::{rows, QUIET};

    /// A short address a code is drawn for throughout.
    fn an_address() -> String {
        "http://192.168.1.10:5055".to_owned()
    }

    #[test]
    fn the_ink_is_the_light_modules_so_a_dark_terminal_reads_right_way_round() {
        let drawn = rows(&an_address(), false).unwrap_or_default();

        // The first line is entirely margin, and the margin is painted. A drawing
        // that left it as background would open with an empty line.
        let top = drawn.first().map(String::as_str).unwrap_or_default();
        assert!(
            !top.is_empty() && top.chars().all(|mark| mark == '\u{2588}'),
            "the quiet zone is not painted, so the code borrows the terminal's own \
             background as its margin: {top:?}"
        );
    }

    #[test]
    fn a_code_carries_a_margin_on_every_side() {
        let drawn = rows(&an_address(), true).unwrap_or_default();

        // Counted off the drawing rather than taken from the constant, so this reads
        // what was produced instead of restating what was asked for.
        let lit_through = drawn
            .iter()
            .take_while(|line| line.chars().all(|mark| mark == '#'))
            .count();
        assert!(
            i32::try_from(lit_through).is_ok_and(|deep| deep >= QUIET),
            "the margin above the code is {lit_through} rows, and a reader needs \
             {QUIET} to find the edge against whatever is printed beside it"
        );
        assert!(
            drawn
                .iter()
                .all(|line| line.starts_with("##") && line.ends_with("##")),
            "a line reaches the edge of the drawing, so the code has no margin beside it"
        );
    }

    /// The two drawings are the same code at two sizes, not two shapes.
    ///
    /// One module is one cell across and half a cell down in the stacked drawing,
    /// and two cells across and one down in the wide one — so the wide drawing is
    /// twice the stacked one in *both* directions. A square code stays square in
    /// each, which is what a camera needs; what differs is only how much screen it
    /// takes.
    #[test]
    fn the_wide_drawing_is_the_stacked_one_at_twice_the_size() {
        let stacked = rows(&an_address(), false).unwrap_or_default();
        let wide = rows(&an_address(), true).unwrap_or_default();
        let across = |drawn: &[String]| drawn.first().map_or(0, |line| line.chars().count());

        assert_eq!(
            stacked.len(),
            wide.len().div_ceil(2),
            "the two drawings disagree about how many module rows the code has"
        );
        assert_eq!(
            across(&wide),
            across(&stacked) * 2,
            "the wide drawing is not twice the width, so one of the two is stretched \
             and a camera has that to correct for"
        );
    }

    #[test]
    fn a_code_has_both_lit_and_unlit_cells() {
        let drawn = rows(&an_address(), false).unwrap_or_default();
        let all: String = drawn.concat();

        assert!(
            all.contains('\u{2588}'),
            "nothing is lit, so this is not a code"
        );
        assert!(
            all.contains(' '),
            "nothing is dark, so this is a filled rectangle rather than a code"
        );
    }

    #[test]
    fn an_address_too_long_to_encode_is_no_code_rather_than_a_wrong_one() {
        let far_too_long = "h".repeat(8000);

        assert!(
            rows(&far_too_long, false).is_none(),
            "an address that does not fit in a code was drawn as one anyway"
        );
    }
}
