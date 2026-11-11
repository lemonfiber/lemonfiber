use super::{asked_noun, TitleKind, TitleUnit};
use crate::collected::{Medium, Unit};
use crate::ports::media::Kind;

#[test]
fn music_and_books_are_named_by_the_units_they_are_asked_for_in() {
    assert_eq!(TitleKind::from(Medium::Music), TitleKind::Music);
    assert_eq!(TitleKind::from(Medium::Books), TitleKind::Books);
    assert_eq!(asked_noun(TitleKind::Music), "album and artist");
    assert_eq!(asked_noun(TitleKind::Books), "book, author and series");
    assert_eq!(asked_noun(TitleKind::from(Kind::Tv)), Kind::Tv.noun());
    assert_eq!(
        asked_noun(TitleKind::from(Kind::Movies)),
        Kind::Movies.noun()
    );
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
