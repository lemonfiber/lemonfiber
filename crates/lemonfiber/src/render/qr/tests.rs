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
