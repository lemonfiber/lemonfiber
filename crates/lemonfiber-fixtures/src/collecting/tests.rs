use super::*;

/// A curator of music answers every question with one candidate and records each.
#[tokio::test]
async fn a_curator_of_music_answers_each_question_with_one_candidate() {
    let curator = Curator::holding(true);
    let ask = MusicAsk {
        unit: MusicUnit::Album,
        reference: "mbid-1".to_owned(),
        root_folder: "/library".to_owned(),
        quality_profile: 3,
        metadata_profile: 3,
        search: true,
    };

    let looked = MusicCurating::lookup(&*curator, MusicUnit::Album, "blue", None).await;
    let one = MusicCurating::candidate(&*curator, MusicUnit::Album, "mbid-1").await;
    let asked = MusicCurating::request(&*curator, &ask).await;
    let profiles = MusicCurating::profiles(&*curator).await;

    assert!(looked.is_ok_and(|page| page.found.len() == 1));
    assert!(one.is_ok_and(|one| one.is_some()));
    assert!(asked.is_ok_and(|asked| asked.reference == "mbid-1"));
    assert!(profiles.is_ok_and(|with| with.metadata.len() == 1));
    assert_eq!(
        curator.asked(),
        [
            "lookup Album blue",
            "candidate Album mbid-1",
            "request Album mbid-1 /library 3 3 true"
        ]
    );
}

/// A curator of books finds nothing and holds no candidate, but takes an ask.
#[tokio::test]
async fn a_curator_of_books_finds_nothing_and_takes_an_ask() {
    let curator = Curator::holding(false);
    let ask = BookAsk {
        unit: BookUnit::Series,
        reference: "ol-1".to_owned(),
        root_folder: "/library".to_owned(),
        quality_profile: 3,
        search: true,
    };

    let looked = BookCurating::lookup(&*curator, BookUnit::Series, "harbour", None).await;
    let one = BookCurating::candidate(&*curator, BookUnit::Series, "ol-1").await;
    let asked = BookCurating::request(&*curator, &ask).await;
    let profiles = BookCurating::profiles(&*curator).await;

    assert!(looked.is_ok_and(|page| page.found.is_empty()));
    assert!(one.is_ok_and(|one| one.is_none()));
    assert!(asked.is_ok_and(|asked| asked.reference == "ol-1"));
    assert!(profiles.is_ok_and(|with| with.metadata.is_empty()));
    assert_eq!(curator.asked().len(), 3);
}
