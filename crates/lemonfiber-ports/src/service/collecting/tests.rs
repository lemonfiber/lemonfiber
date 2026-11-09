use super::{BookUnit, CuratedUnit, MusicUnit};

#[test]
fn every_unit_of_music_and_books_is_its_own_curated_unit() {
    let curated: Vec<CuratedUnit> = [MusicUnit::Album, MusicUnit::Artist]
        .map(CuratedUnit::from)
        .into_iter()
        .chain([BookUnit::Book, BookUnit::Author, BookUnit::Series].map(CuratedUnit::from))
        .collect();

    assert_eq!(
        curated,
        [
            CuratedUnit::Album,
            CuratedUnit::Artist,
            CuratedUnit::Book,
            CuratedUnit::Author,
            CuratedUnit::Series,
        ]
    );
    assert_eq!(
        serde_json::to_string(&curated).ok().as_deref(),
        Some(r#"["album","artist","book","author","series"]"#)
    );
}
