use super::{Medium, NotAUnit, Reference, Unit};
use crate::ports::service::{BookUnit, MusicUnit};

#[test]
fn a_request_service_number_reads_and_writes_bare() {
    assert_eq!("42".parse(), Ok(Reference::Filed(42)));
    assert_eq!(Reference::Filed(42).to_string(), "42");
}

#[test]
fn a_kept_ask_reads_and_writes_with_its_medium() {
    assert_eq!("music-7".parse(), Ok(Reference::Kept(Medium::Music, 7)));
    assert_eq!(" books-3 ".parse(), Ok(Reference::Kept(Medium::Books, 3)));
    assert_eq!(Reference::Kept(Medium::Books, 3).to_string(), "books-3");
}

#[test]
fn anything_else_names_no_request() {
    for written in [
        "",
        "-",
        "music-",
        "music-0",
        "music--1",
        "music-x",
        "film-1",
        "4 2",
        "+42",
        "-42",
        "music-1-2",
    ] {
        assert!(written.parse::<Reference>().is_err(), "{written}");
    }
}

#[test]
fn a_whole_artist_author_or_series_always_waits_and_an_album_or_book_does_not() {
    assert!(Unit::Music(MusicUnit::Artist).always_waits());
    assert!(Unit::Books(BookUnit::Author).always_waits());
    assert!(Unit::Books(BookUnit::Series).always_waits());
    assert!(!Unit::Music(MusicUnit::Album).always_waits());
    assert!(!Unit::Books(BookUnit::Book).always_waits());
}

#[test]
fn each_unit_is_of_its_medium() {
    assert_eq!(Unit::Music(MusicUnit::Album).medium(), Medium::Music);
    assert_eq!(Unit::Books(BookUnit::Series).medium(), Medium::Books);
}

#[test]
fn a_reference_is_kept_as_the_words_it_is_named_by() {
    let written = serde_json::to_string(&Reference::Kept(Medium::Music, 7));
    assert_eq!(written.ok().as_deref(), Some("\"music-7\""));
    assert_eq!(
        serde_json::from_str::<Reference>("\"42\"").ok(),
        Some(Reference::Filed(42))
    );
    assert!(serde_json::from_str::<Reference>("\"film-1\"").is_err());
    assert!(serde_json::from_str::<Reference>("42").is_err());
}

#[test]
fn a_kind_alone_chooses_its_medium_s_first_unit_and_a_unit_settles_its_medium() {
    assert_eq!(Unit::chosen(None, None), Ok(None));
    assert_eq!(Unit::chosen(Some("tv"), None), Ok(None));
    assert_eq!(
        Unit::chosen(Some("music"), None),
        Ok(Some(Unit::Music(MusicUnit::Album)))
    );
    assert_eq!(
        Unit::chosen(Some("books"), None),
        Ok(Some(Unit::Books(BookUnit::Book)))
    );
    assert_eq!(
        Unit::chosen(None, Some("series")),
        Ok(Some(Unit::Books(BookUnit::Series)))
    );
    assert_eq!(
        Unit::chosen(Some("music"), Some("artist")),
        Ok(Some(Unit::Music(MusicUnit::Artist)))
    );
}

#[test]
fn a_unit_of_the_other_medium_or_of_none_chooses_nothing() {
    assert_eq!(Unit::chosen(Some("music"), Some("book")), Err(NotAUnit));
    assert_eq!(Unit::chosen(Some("books"), Some("album")), Err(NotAUnit));
    assert_eq!(Unit::chosen(None, Some("season")), Err(NotAUnit));
}

#[test]
fn every_unit_is_named_by_a_word_of_its_own_and_a_household_noun() {
    let words: std::collections::BTreeSet<&str> =
        Unit::ALL.iter().map(|unit| unit.word()).collect();
    assert_eq!(words.len(), Unit::ALL.len());
    for unit in Unit::ALL {
        assert_eq!(Unit::chosen(None, Some(unit.word())), Ok(Some(unit)));
        assert!(!unit.noun().is_empty());
    }
    assert_eq!(Unit::Books(BookUnit::Series).noun(), "book series");
}
