use super::{askable, leaves_of};
use crate::collected::store::{Ask, Standing};
use crate::collected::Unit;
use crate::model::{Stands, TitleKind, TitleUnit};
use crate::ports::service::{BookUnit, Candidate, CandidateState, Left, MusicUnit};

const ALBUM: Unit = Unit::Music(MusicUnit::Album);

fn candidate(reference: &str, state: CandidateState) -> Candidate {
    Candidate {
        reference: reference.to_owned(),
        name: "Kind of Blue".to_owned(),
        by: Some("Miles Davis".to_owned()),
        year: Some(1959),
        cover: Some("https://covers.example/1.jpg".to_owned()),
        format: None,
        state,
    }
}

fn waiting(unit: Unit, reference: &str, standing: Standing) -> Ask {
    Ask {
        number: 1,
        unit,
        reference: reference.to_owned(),
        name: String::new(),
        by: None,
        year: None,
        member: "ada".to_owned(),
        asked_at: String::new(),
        standing,
        settled_at: None,
        arrived_at: None,
    }
}

#[test]
fn a_candidate_is_shown_with_its_artist_cover_and_unit_by_the_curator_s_reference() {
    let shown = askable(ALBUM, candidate("mbid-1", CandidateState::MayAsk), &[]);
    assert_eq!(shown.id, "mbid-1");
    assert_eq!(shown.kind, TitleKind::Music);
    assert_eq!(shown.unit, Some(TitleUnit::Album));
    assert_eq!(shown.by.as_deref(), Some("Miles Davis"));
    assert_eq!(shown.year, Some(1959));
    assert!(shown.cover.is_some());
    assert_eq!(shown.stands, Stands::Askable);
}

#[test]
fn an_artist_held_in_part_is_partly_here_until_an_ask_for_the_rest_waits() {
    let artist = Unit::Music(MusicUnit::Artist);
    assert_eq!(
        askable(artist, candidate("a", CandidateState::PartlyHere), &[]).stands,
        Stands::PartlyHere
    );
    assert_eq!(
        askable(
            artist,
            candidate("a", CandidateState::PartlyHere),
            &[waiting(artist, "a", Standing::Waiting)]
        )
        .stands,
        Stands::OnItsWay
    );
}

#[test]
fn a_candidate_stands_where_the_curator_says_unless_an_ask_for_it_waits_here() {
    assert_eq!(
        askable(ALBUM, candidate("a", CandidateState::Here), &[]).stands,
        Stands::Here
    );
    assert_eq!(
        askable(ALBUM, candidate("a", CandidateState::Asked), &[]).stands,
        Stands::OnItsWay
    );
    let kept = [
        waiting(ALBUM, "a", Standing::Waiting),
        waiting(ALBUM, "b", Standing::Declined),
        waiting(Unit::Music(MusicUnit::Artist), "c", Standing::Waiting),
    ];
    assert_eq!(
        askable(ALBUM, candidate("a", CandidateState::MayAsk), &kept).stands,
        Stands::OnItsWay
    );
    assert_eq!(
        askable(ALBUM, candidate("b", CandidateState::MayAsk), &kept).stands,
        Stands::Askable
    );
    assert_eq!(
        askable(ALBUM, candidate("c", CandidateState::MayAsk), &kept).stands,
        Stands::Askable
    );
}

#[test]
fn what_is_left_is_said_for_the_asked_medium_alone() {
    let left = Left {
        limit: Some(3),
        used: 1,
        days: Some(7),
    };
    let music = leaves_of(ALBUM, left, 1);
    assert_eq!(
        (music.music, music.books, music.films),
        (Some(1), None, None)
    );
    let books = leaves_of(Unit::Books(BookUnit::Book), left, 1);
    assert_eq!((books.music, books.books), (None, Some(1)));
    let none = Left {
        limit: None,
        used: 0,
        days: None,
    };
    assert_eq!(leaves_of(ALBUM, none, 1).music, None);
}

mod arriving;
mod deciding;
mod flows;
