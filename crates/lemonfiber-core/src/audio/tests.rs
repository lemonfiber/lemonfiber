use super::Format;

#[test]
fn the_default_is_a_lossless_copy_of_the_cd() {
    // The reason to keep a library rather than stream it — the middle of the three,
    // chosen when the operator says nothing.
    assert_eq!(Format::default_format(), Format::Lossless);
}

#[test]
fn every_format_round_trips_through_its_plain_label() {
    for format in Format::ALL {
        assert_eq!(Format::from_label(format.label()), Some(format));
    }
}

#[test]
fn a_name_that_is_not_a_format_is_refused_rather_than_guessed() {
    assert_eq!(Format::from_label("lossy"), None);
    assert_eq!(Format::from_label(""), None);
}

#[test]
fn a_label_is_plain_language_with_no_format_or_bitrate_jargon() {
    // The operator selects by "how good", never by a codec or a bitrate — the same
    // plain-language contract the resolution presets keep.
    for format in Format::ALL {
        let label = format.label();
        assert!(!label.is_empty());
        assert!(!label.contains("FLAC"));
        assert!(!label.contains("MP3"));
        assert!(!label.contains("kbps"));
        assert!(!label.contains("bit"));
    }
}

#[test]
fn each_format_states_a_meaning_a_size_and_a_caveat() {
    for format in Format::ALL {
        assert!(!format.means().is_empty());
        let consequence = format.consequence();
        assert!(!consequence.format.is_empty());
        assert!(
            consequence.size_per_hour.contains("MB") || consequence.size_per_hour.contains("GB"),
            "{} names no size",
            consequence.size_per_hour
        );
        assert!(!consequence.note.is_empty());
    }
}

#[test]
fn the_formats_are_offered_least_to_most_demanding() {
    // Compact, then lossless, then hi-res — the order the surface presents, and the
    // order that makes "most demanding" meaningful for a future projection.
    assert_eq!(
        Format::ALL,
        [Format::Compact, Format::Lossless, Format::HiRes]
    );
}

#[test]
fn a_format_serialises_under_its_plain_label() {
    // The stored form is the label the operator chose, not an integer or a codec —
    // the choice file reads as the question it answered.
    let json = serde_json::to_string(&Format::HiRes).unwrap_or_default();
    assert_eq!(json, r#""hi-res""#);
    let back: Option<Format> = serde_json::from_str(&json).ok();
    assert_eq!(back, Some(Format::HiRes));
}
