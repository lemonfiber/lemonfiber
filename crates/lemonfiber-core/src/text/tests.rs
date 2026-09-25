use super::{escaped, fitted, plain, wrapped, Overrun};

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

/// What serialising leaves for somebody else to deal with.
///
/// The claim this door was built on — that JSON escapes every control character
/// — is true only below a space. Asserted against the serialiser itself rather
/// than restated, because the whole defect was a sentence about `serde_json`
/// that nothing checked: this fails the day it starts escaping them, which is
/// the day the door could stop.
#[test]
fn serialising_carries_the_ones_above_a_space_raw() {
    for carried in ['\u{9b}', '\u{2028}', '\u{202e}', '\u{200b}', '\u{feff}'] {
        let written = serde_json::to_string(&format!("Some{carried}Release")).unwrap_or_default();
        assert!(
            written.contains(carried),
            "{carried:?} arrives raw: {written:?}"
        );
    }
    let below = serde_json::to_string("Some\u{1b}Release").unwrap_or_default();
    assert!(!below.contains('\u{1b}'), "and below a space it does not");
}

/// A document printed to a terminal carries no instruction for it.
///
/// The same names the plain-text tests above are written from, put through the
/// other door. What comes out is a document, so the check is that nothing a
/// terminal obeys survives in it — not that the characters are gone from the
/// value, which is the next test.
#[test]
fn the_parsers_door_leaves_nothing_a_terminal_obeys() {
    for hidden in [
        '\u{1b}', '\u{9b}', '\u{7f}', '\u{2028}', '\u{2029}', '\u{202a}', '\u{202b}', '\u{202c}',
        '\u{202d}', '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}', '\u{200b}',
        '\u{feff}',
    ] {
        let written = serde_json::to_string(&format!("Some{hidden}Release")).unwrap_or_default();
        let out = escaped(&written);
        assert!(
            !out.contains(hidden),
            "{hidden:?} still reaches the terminal: {out:?}"
        );
    }
}

/// And the value a script is handed is the one the service holds.
///
/// The reason this door escapes rather than makes plain. A name with the
/// character removed no longer matches what it was taken from, and matching it
/// is what a script asked for the document to do.
#[test]
fn what_a_parser_reads_back_is_the_name_it_was_sent() {
    for hidden in ['\u{9b}', '\u{2028}', '\u{202e}', '\u{200b}', '\u{feff}'] {
        let name = format!("Some{hidden}Release");
        let written = serde_json::to_string(&name).unwrap_or_default();
        let read: String = serde_json::from_str(&escaped(&written)).unwrap_or_default();
        assert_eq!(read, name, "{hidden:?} did not survive the door");
    }
}

/// The two doors answer about the same set.
///
/// The drift this is bought against: a character recognised as an instruction on
/// one door and not on the other leaves the quieter door open, and nothing about
/// either function would look wrong. Every code point below the astral planes is
/// asked of both, so a range added to one and not the other fails here.
#[test]
fn a_character_one_door_answers_for_is_answered_by_the_other() {
    // Filtered rather than pushed into from a branch. A branch that only runs
    // where the doors disagree is a line nothing executes while they agree, and
    // the coverage gate reads test code too.
    let disagreeing: Vec<u32> = (0..=0xffff_u32)
        .filter_map(char::from_u32)
        .filter(|character| {
            let name = format!("a{character}b");
            let removed = !plain(&name).contains(*character);
            let written_out = !escaped(&name).contains(*character);
            removed != (written_out || super::laid_out(*character))
        })
        .map(u32::from)
        .collect();
    assert!(
        disagreeing.is_empty(),
        "one door treats these as an instruction and the other does not: {disagreeing:?}"
    );
}

/// The three a document may be laid out with are left where they are.
///
/// Between two tokens each of them is whitespace, and an escape there is a
/// document that will not parse. Nothing serialises that way today, which is why
/// this is stated rather than discovered by whoever adds pretty-printing.
#[test]
fn the_characters_a_document_is_laid_out_with_are_left_alone() {
    assert_eq!(escaped("{\n\t\"a\": 1\r\n}"), "{\n\t\"a\": 1\r\n}");
}

#[test]
fn a_document_with_nothing_to_write_out_comes_back_as_it_was() {
    assert_eq!(escaped(""), "");
    assert_eq!(escaped("{\"name\":\"Amélie\"}"), "{\"name\":\"Amélie\"}");
}

/// An escape is written the way JSON spells one.
///
/// Four hex digits, lower case, so what is produced is the same text the
/// serialiser would have produced had it written the character out itself.
#[test]
fn an_escape_is_written_the_way_the_serialiser_writes_one() {
    assert_eq!(escaped("a\u{202e}b"), "a\\u202eb");
    assert_eq!(escaped("a\u{9b}b"), "a\\u009bb");
    assert_eq!(escaped("a\u{feff}b"), "a\\ufeffb");
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
