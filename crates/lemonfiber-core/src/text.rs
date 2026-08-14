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
const fn obeyed(character: char) -> bool {
    matches!(character, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}' | '\u{2028}' | '\u{2029}')
}

#[cfg(test)]
mod tests {
    use super::plain;

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
        ] {
            assert_eq!(plain(kept), kept);
        }
    }

    #[test]
    fn text_with_nothing_to_remove_comes_back_as_it_was() {
        assert_eq!(plain(""), "");
        assert_eq!(plain("plain"), "plain");
    }
}
