//! `music.curate` and `books.curate` across their contracts: one script run against a
//! curator in process and through each contract, which must answer alike and be told alike.

use lemonfiber_fixtures::upstream::collecting::UNHELD;
use lemonfiber_ports::service::{
    BookAsk, BookCurating, BookUnit, Failure, MusicAsk, MusicCurating, MusicUnit,
};

use super::{books, crosses_alike, music};

/// A result as the script writes it down: a failure by its kind and words alone.
fn said<T: std::fmt::Debug>(result: Result<T, Failure>) -> String {
    match result {
        Ok(answer) => format!("{answer:?}"),
        Err(Failure::Refused { detail, .. }) => format!("refused: {detail}"),
        Err(failure) => format!("{failure}"),
    }
}

/// Every operation of `music.curate`, each answer written down.
async fn listening<C: MusicCurating>(curator: &C) -> Vec<String> {
    let ask = |unit, reference: &str| MusicAsk {
        unit,
        reference: reference.to_owned(),
        root_folder: "/data/media/music".to_owned(),
        quality_profile: 1,
        metadata_profile: 1,
        search: true,
    };
    vec![
        said(curator.lookup(MusicUnit::Album, "harbour", None).await),
        said(
            curator
                .lookup(MusicUnit::Artist, "lanterns", Some("2"))
                .await,
        ),
        said(curator.candidate(MusicUnit::Album, "album-0d1e").await),
        said(curator.candidate(MusicUnit::Artist, UNHELD).await),
        said(curator.request(&ask(MusicUnit::Album, "album-0d1e")).await),
        said(curator.request(&ask(MusicUnit::Artist, UNHELD)).await),
        said(curator.profiles().await),
        said(curator.remove(MusicUnit::Album, "album-0d1e").await),
        said(curator.remove(MusicUnit::Artist, UNHELD).await),
    ]
}

/// Every operation of `books.curate`, each answer written down.
async fn reading<C: BookCurating>(curator: &C) -> Vec<String> {
    let ask = |unit, reference: &str| BookAsk {
        unit,
        reference: reference.to_owned(),
        root_folder: "/data/media/books".to_owned(),
        quality_profile: 1,
        search: false,
    };
    vec![
        said(curator.lookup(BookUnit::Book, "harbour", None).await),
        said(curator.lookup(BookUnit::Series, "lights", Some("2")).await),
        said(curator.candidate(BookUnit::Series, "series-0d1e").await),
        said(curator.candidate(BookUnit::Book, UNHELD).await),
        said(curator.request(&ask(BookUnit::Author, "author-0d1e")).await),
        said(curator.request(&ask(BookUnit::Book, UNHELD)).await),
        said(curator.profiles().await),
        said(curator.remove(BookUnit::Series, "series-0d1e").await),
        said(curator.remove(BookUnit::Book, UNHELD).await),
    ]
}

#[tokio::test]
async fn a_curator_of_music_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        listening,
        music::curate::Adapter,
        [
            "lookup",
            "lookup",
            "candidate",
            "candidate",
            "request",
            "request",
            "profiles",
            "remove",
            "remove"
        ]
    );
}

#[tokio::test]
async fn a_curator_of_books_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        reading,
        books::curate::Adapter,
        [
            "lookup",
            "lookup",
            "candidate",
            "candidate",
            "request",
            "request",
            "profiles",
            "remove",
            "remove"
        ]
    );
}

#[test]
fn a_request_for_a_book_waits_for_the_curator_to_fetch_its_record() {
    let declared = books::curate::capability()
        .operations
        .iter()
        .map(|operation| (operation.name, operation.bounds.deadline))
        .filter(|(_, deadline)| *deadline != crate::client::DEADLINE)
        .collect::<Vec<_>>();
    assert_eq!(declared, [("request", books::REQUEST_DEADLINE)]);
    assert_eq!(books::REQUEST_DEADLINE, std::time::Duration::from_secs(60));
}
