use std::sync::Arc;

use lemonfiber_fixtures::collecting::Curator;

use super::Collector;
use crate::collected::Unit;
use crate::ports::service::{BookUnit, Failure, MusicUnit};

const ALBUM: Unit = Unit::Music(MusicUnit::Album);

const SERIES: Unit = Unit::Books(BookUnit::Series);

#[tokio::test]
async fn a_curator_of_music_is_asked_for_music_at_its_first_folder_and_profiles_searching() {
    let curator = Curator::holding(true);
    let collector = Collector::Music(curator.clone());

    let page = collector.lookup(ALBUM, "blue", None).await;
    let one = collector.candidate(ALBUM, "mbid-1").await;
    let asked = collector.request(ALBUM, "mbid-1").await;

    assert!(page.is_ok_and(|page| page.found.len() == 1));
    assert!(one.is_ok_and(|one| one.is_some()));
    assert!(asked.is_ok());
    assert_eq!(
        curator.asked(),
        [
            "lookup Album blue",
            "candidate Album mbid-1",
            "request Album mbid-1 /library 3 3 true"
        ]
    );
}

#[tokio::test]
async fn a_curator_of_books_is_asked_for_books_at_its_first_folder_and_profile_searching() {
    let curator = Curator::holding(false);
    let collector = Collector::Books(curator.clone());

    let page = collector.lookup(SERIES, "harbour", None).await;
    let one = collector.candidate(SERIES, "ol-1").await;
    let asked = collector.request(SERIES, "ol-1").await;

    assert!(page.is_ok());
    assert!(one.is_ok_and(|one| one.is_none()));
    assert!(asked.is_ok());
    assert_eq!(
        curator.asked(),
        [
            "lookup Series harbour",
            "candidate Series ol-1",
            "request Series ol-1 /library 3 true"
        ]
    );
}

#[tokio::test]
async fn a_unit_of_the_other_medium_is_not_asked_of_a_curator() {
    let curator = Curator::holding(true);
    let music = Collector::Music(curator.clone());
    let books = Collector::Books(curator.clone());

    let unsupported =
        |result: Result<_, Failure>| matches!(result, Err(Failure::Unsupported { .. }));
    assert!(unsupported(
        music.lookup(SERIES, "harbour", None).await.map(|_| ())
    ));
    assert!(unsupported(
        music.candidate(SERIES, "ol-1").await.map(|_| ())
    ));
    assert!(unsupported(music.request(SERIES, "ol-1").await.map(|_| ())));
    assert!(unsupported(
        books.lookup(ALBUM, "blue", None).await.map(|_| ())
    ));
    assert!(curator.asked().is_empty());
}

#[tokio::test]
async fn a_curator_holding_nothing_to_ask_with_is_not_asked() {
    let bare = Arc::new(Curator::default());
    let without_metadata = Curator::holding(false);

    let none = Collector::Books(bare.clone()).request(SERIES, "ol-1").await;
    let no_metadata = Collector::Music(without_metadata.clone())
        .request(ALBUM, "mbid-1")
        .await;

    assert!(matches!(none, Err(Failure::Refused { .. })));
    assert!(matches!(no_metadata, Err(Failure::Refused { .. })));
    assert!(bare.asked().is_empty() && without_metadata.asked().is_empty());
}
