use super::{Medium, Reference, Unit};
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
