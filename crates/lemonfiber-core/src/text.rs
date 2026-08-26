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
//! Beside it live the two other things that happen to text on its way to being
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
mod tests {
    use super::{fitted, plain, wrapped, Overrun};

    #[test]
    fn a_release_name_that_would_clear_the_screen_no_longer_can() {
        // The case this exists for. An indexer supplies the name; a terminal reads
        // the escape; the screen stops saying what this product said.
        assert_eq!(plain("Some\u{1b}[2JRelease"), "Some[2JRelease");
    }

    #[test]
    fn a_carriage_return_cannot_write_over_the_line_before_it() {
        assert_eq!(plain("harmless\rHACKED"), "harmlessHACKED");
    }

    #[test]
    fn a_newline_cannot_forge_a_second_line() {
        // One line in means one line out: a message that could add lines could add
        // a line that looks like this product's own.
        assert_eq!(plain("one\ntwo"), "onetwo");
    }

    #[test]
    fn the_single_code_point_form_of_an_escape_is_caught_too() {
        // C1 as one code point, which is what survives a round trip through a
        // service's JSON — and what `is_control` alone would let through.
        assert_eq!(plain("a\u{9b}2Jb"), "a2Jb");
    }

    #[test]
    fn a_line_separator_that_moves_a_cursor_is_removed() {
        assert_eq!(plain("a\u{2028}b"), "ab");
    }

    #[test]
    fn an_override_can_no_longer_reverse_the_name_it_precedes() {
        // The classic one: a right-to-left override in front of the extension draws
        // `Some.Releaseexe.jpg` on the screen while the queue holds `...gpj.exe`.
        // The characters either side of it are kept, so what is drawn is the name.
        let disguised = "Some.Release\u{202e}gpj.exe";
        let shown = plain(disguised);
        assert!(!shown.contains('\u{202e}'), "{shown:?}");
        assert_eq!(shown, "Some.Releasegpj.exe");
    }

    #[test]
    fn every_shape_of_reordering_and_every_invisible_space_goes() {
        // Each of these draws no glyph of its own and changes what the glyphs
        // around it say: five embeddings and overrides, four isolates, and the two
        // spaces that occupy no cell at all.
        for hidden in [
            '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}', '\u{2067}',
            '\u{2068}', '\u{2069}', '\u{200b}', '\u{feff}',
        ] {
            let name = format!("Some{hidden}Release");
            assert_eq!(plain(&name), "SomeRelease", "U+{:04X}", u32::from(hidden));
        }
    }

    #[test]
    fn everything_a_person_could_have_meant_survives() {
        // A release name in another script is a release name. Stripping more than
        // a terminal obeys would corrupt the very names this is meant to show.
        for kept in [
            "Some.Release.2160p",
            "Amélie",
            "千と千尋の神隠し",
            "Что-то",
            "a — b · c",
            "🎬 premiere",
            "path/with spaces & (brackets)",
            // Right-to-left text needs no override to be drawn the way it reads,
            // and the marks that settle a neutral character's direction are not
            // overrides. Dropping these would misspell a name rather than disarm it.
            "الحلقة الأولى",
            "\u{200f}العنوان\u{200e} (2024)",
            // A joiner is what makes one emoji out of four, and what keeps a
            // Persian word's letters apart without a space between them.
            "👩‍👩‍👧‍👦 family",
            "می\u{200c}شود",
        ] {
            assert_eq!(plain(kept), kept);
        }
    }

    #[test]
    fn text_with_nothing_to_remove_comes_back_as_it_was() {
        assert_eq!(plain(""), "");
        assert_eq!(plain("plain"), "plain");
    }

    /// The ordinary case, and the one both callers are built on.
    #[test]
    fn text_is_broken_at_spaces_and_no_line_runs_past_the_width() {
        let broken = wrapped("the quick brown fox jumps", 10, Overrun::Broken);

        assert_eq!(broken, ["the quick", "brown fox", "jumps"]);
        assert!(broken.iter().all(|line| line.chars().count() <= 10));
    }

    /// Shorter than the width is one line, and nothing at all is no lines.
    #[test]
    fn text_that_already_fits_is_left_alone() {
        assert_eq!(wrapped("short", 40, Overrun::Broken), ["short"]);
        assert!(wrapped("", 40, Overrun::Broken).is_empty());
        assert!(wrapped("anything", 0, Overrun::Broken).is_empty());
    }

    /// A service that lined its own output up with spaces meant them, so what is
    /// inside a line arrives as it was written.
    #[test]
    fn spaces_inside_a_line_survive_the_break() {
        assert_eq!(
            wrapped("a  b  c  ddddd", 8, Overrun::Broken),
            ["a  b  c", "ddddd"]
        );
    }

    /// The difference between the two edges, on the one input that tells them
    /// apart: a run with nothing in it to break at.
    #[test]
    fn a_run_with_nothing_to_break_on_answers_to_the_edge_it_was_given() {
        let path = format!("saw {}", "x".repeat(20));

        assert_eq!(
            wrapped(&path, 10, Overrun::Allowed),
            ["saw", &"x".repeat(20)],
            "a report is re-wrapped by the terminal reading it"
        );
        assert_eq!(
            wrapped(&path, 10, Overrun::Broken),
            ["saw", &"x".repeat(10), &"x".repeat(10)],
            "a screen is a grid, and past the edge is not drawn"
        );
    }

    /// A run past the edge is kept whole rather than run to the end of the text.
    #[test]
    fn an_overrun_that_is_allowed_still_ends_where_the_run_does() {
        assert_eq!(
            wrapped(
                &format!("{} and more", "x".repeat(20)),
                10,
                Overrun::Allowed
            ),
            ["x".repeat(20), "and more".to_owned()]
        );
    }

    /// Leading whitespace has nothing before it to end a line at, and a break taken
    /// there would take nothing at all.
    #[test]
    fn text_that_starts_with_a_space_still_makes_progress() {
        assert_eq!(
            wrapped("  abcdefgh", 4, Overrun::Broken),
            ["  ab", "cdef", "gh"]
        );
    }

    /// Width is counted in characters rather than in bytes: a name in another
    /// script is measured by what a terminal draws, not by what it stores.
    #[test]
    fn a_line_is_measured_in_what_a_terminal_draws() {
        assert_eq!(
            wrapped("Amélie Amélie Amélie", 13, Overrun::Broken),
            ["Amélie Amélie", "Amélie"]
        );
    }
    /// A name that fits is left exactly as it is — shortening one that needs no
    /// shortening would be inventing a change to it.
    #[test]
    fn text_that_already_fits_the_row_is_left_alone() {
        assert_eq!(fitted("Short.Name", 40), "Short.Name");
        assert_eq!(fitted(&"x".repeat(40), 40), "x".repeat(40));
    }

    /// The defect this exists for: cut at the tail, two releases that differ only in
    /// resolution read identically, and a list of what is downloading that cannot
    /// tell them apart fails at the one question it exists to answer.
    #[test]
    fn two_values_differing_only_at_the_end_stay_apart() {
        let hd = "A.Very.Long.Release.Name.From.Some.Group.2024.1080p.WEB-DL";
        let uhd = "A.Very.Long.Release.Name.From.Some.Group.2024.2160p.WEB-DL";

        let lesser = fitted(hd, 40);
        let better = fitted(uhd, 40);
        assert_ne!(lesser, better, "both were shortened to the same thing");
        assert!(better.ends_with("WEB-DL"), "{better}");
        assert!(better.starts_with("A.Very.Long"), "{better}");
    }

    /// Never wider than asked for, however it was shortened.
    #[test]
    fn a_shortened_value_still_fits_the_row() {
        for width in [4, 7, 20, 41] {
            let shortened = fitted(&"z".repeat(120), width);
            assert_eq!(shortened.chars().count(), width, "{shortened}");
        }
    }

    /// The marker is full stops rather than an ellipsis, so a terminal that cannot
    /// render the character is never handed one.
    #[test]
    fn shortening_uses_no_character_a_terminal_might_not_have() {
        let text = fitted(&"y".repeat(80), 40);

        assert!(text.contains("..."), "{text}");
        assert!(text.is_ascii(), "{text}");
    }

    /// A row too narrow for both ends and a marker says only that something is
    /// there: a half of a name is read as a name, and a marker is not.
    #[test]
    fn a_row_with_no_room_for_both_ends_keeps_neither() {
        assert_eq!(fitted("Some.Release.2160p", 3), "...");
        assert_eq!(fitted("Some.Release.2160p", 2), "..");
        assert_eq!(fitted("Some.Release.2160p", 0), "");
    }

    /// Width is counted in characters rather than in bytes, on both ends of it.
    #[test]
    fn a_shortened_value_is_measured_in_what_a_terminal_draws() {
        assert_eq!(fitted("Amélie.Amélie.Amélie", 11), "Amél...élie");
    }
}
