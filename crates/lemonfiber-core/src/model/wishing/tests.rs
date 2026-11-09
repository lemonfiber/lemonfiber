use super::{TitleKind, TitleUnit};
use crate::collected::{Medium, Unit};
use crate::ports::media::Kind;

#[test]
fn music_and_books_are_named_by_the_units_they_are_asked_for_in() {
    assert_eq!(TitleKind::from(Medium::Music), TitleKind::Music);
    assert_eq!(TitleKind::from(Medium::Books), TitleKind::Books);
    assert_eq!(TitleKind::Music.noun(), "album and artist");
    assert_eq!(TitleKind::Books.noun(), "book, author and series");
    assert_eq!(TitleKind::from(Kind::Tv).noun(), Kind::Tv.noun());
    assert_eq!(TitleKind::from(Kind::Movies).noun(), Kind::Movies.noun());
}

#[test]
fn each_unit_is_shown_as_the_household_calls_it() {
    let shown: Vec<(TitleUnit, &str)> = Unit::ALL
        .into_iter()
        .map(TitleUnit::from)
        .map(|unit| (unit, unit.noun()))
        .collect();

    assert_eq!(
        shown,
        [
            (TitleUnit::Album, "album"),
            (TitleUnit::Artist, "artist"),
            (TitleUnit::Book, "book"),
            (TitleUnit::Author, "author"),
            (TitleUnit::Series, "book series"),
        ]
    );
}
