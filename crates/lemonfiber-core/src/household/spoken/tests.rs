use crate::app::Outcome;
use crate::asking::{Estimate, Passed, Refused};
use crate::error::codes::play::{MISNAMED, SERVER_SILENT};
use crate::error::codes::quota::UNREACHABLE;
use crate::error::household::{unhoused, Obstacle, Words};
use crate::model::{
    Held, HeldReport, HouseholdMember, HouseholdReport, Located, MemberRequest, PartWay,
    PartWayReport, PlayingReport, Title, TitleReport,
};
use crate::ports::service::Medium;

/// What the operator is told is in the way, worded for them.
fn operator(code: crate::error::Code) -> Obstacle {
    Obstacle::new(code, "the media server at 172.18.0.4:8096 would not say")
}

/// What a member is told is in the way under `code`.
fn household(code: crate::error::Code) -> Obstacle {
    let words = Words::of(code);
    Obstacle {
        code,
        said: words.said.to_owned(),
        remedy: words.remedy.to_owned(),
    }
}

/// A title the media server could not be located at, for the operator.
fn unlocated() -> Held {
    Held {
        id: "0123456789abcdef0123456789abcdef".to_owned(),
        title: "Dune".to_owned(),
        year: None,
        medium: Medium::Film,
        at: Located {
            unlocated: Some(operator(MISNAMED)),
            ..Located::default()
        },
    }
}

/// Every obstacle on a member's shelf, and why each title is not located, is told in the
/// household's words by its code.
#[test]
fn a_members_shelf_is_worded_for_the_household() {
    let Outcome::Held(shelf) = Outcome::Held(HeldReport {
        holdings: vec![unlocated()],
        findings: vec![operator(SERVER_SILENT)],
        ..HeldReport::default()
    })
    .for_household() else {
        panic!("a shelf stopped being a shelf");
    };
    assert_eq!(shelf.findings, vec![household(SERVER_SILENT)]);
    assert_eq!(
        shelf
            .holdings
            .first()
            .and_then(|held| held.at.unlocated.clone()),
        Some(household(MISNAMED))
    );
}

/// What is playing, what was part-way through and a title are worded the same way.
#[test]
fn every_member_read_is_worded_for_the_household() {
    let Outcome::Playing(playing) = Outcome::Playing(PlayingReport {
        findings: vec![operator(SERVER_SILENT)],
        ..PlayingReport::default()
    })
    .for_household() else {
        panic!("what is playing stopped being what is playing");
    };
    assert_eq!(playing.findings, vec![household(SERVER_SILENT)]);

    let Outcome::PartWay(part_way) = Outcome::PartWay(PartWayReport {
        part_way: vec![PartWay {
            held: unlocated(),
            position: 1,
            length: None,
        }],
        findings: vec![operator(SERVER_SILENT)],
        ..PartWayReport::default()
    })
    .for_household() else {
        panic!("what was part-way through stopped being that");
    };
    assert_eq!(part_way.findings, vec![household(SERVER_SILENT)]);
    assert_eq!(
        part_way
            .part_way
            .first()
            .and_then(|one| one.held.at.unlocated.clone()),
        Some(household(MISNAMED))
    );

    let Outcome::Title(titled) = Outcome::Title(TitleReport {
        title: Some(Title {
            held: unlocated(),
            overview: None,
            minutes: None,
            genres: Vec::new(),
            certificate: None,
            released: None,
            seasons: Vec::new(),
        }),
        ..TitleReport::default()
    })
    .for_household() else {
        panic!("a title stopped being a title");
    };
    assert_eq!(
        titled.title.and_then(|title| title.held.at.unlocated),
        Some(household(MISNAMED))
    );
}

/// A member's household row carries none of what is written for the operator: no notes,
/// no caution about limits, no size and nowhere a refusal was passed on to.
#[test]
fn a_members_household_row_carries_nothing_written_for_the_operator() {
    let request = MemberRequest {
        id: "1".to_owned(),
        asked: None,
        title: Some("Dune".to_owned()),
        year: None,
        arrived: None,
        shelf_id: None,
        media: None,
        state: None,
        waiting_days: None,
        estimate: Some(Estimate {
            bytes: 4_000_000_000,
            measured: false,
        }),
        refused: Some(Refused {
            reason: "we have it already".to_owned(),
            at: None,
            told: Some(Passed {
                to: vec!["Pushover".to_owned()],
                at: None,
            }),
            expired: false,
        }),
    };
    let Outcome::Household(row) = Outcome::Household(HouseholdReport {
        members: vec![HouseholdMember {
            requests: vec![request],
            ..HouseholdMember::default()
        }],
        findings: vec![operator(UNREACHABLE)],
        noted: vec!["ana has used their limit".to_owned()],
        filtering: Some("a content filter, not a lock".to_owned()),
        ..HouseholdReport::default()
    })
    .for_household() else {
        panic!("a household row stopped being one");
    };
    assert_eq!(row.findings, vec![household(UNREACHABLE)]);
    assert!(row.noted.is_empty());
    assert_eq!(row.filtering, None);
    let request = row
        .members
        .first()
        .and_then(|member| member.requests.first());
    assert_eq!(request.and_then(|request| request.estimate), None);
    assert_eq!(
        request
            .and_then(|request| request.refused.as_ref())
            .map(|refused| (refused.reason.as_str(), refused.told.is_none())),
        Some(("we have it already", true))
    );
}

/// What a member is told about anything in the way of one of their reads names no
/// service, size, address or state of the house.
#[test]
fn what_a_member_is_told_is_in_the_way_is_in_the_households_words() {
    let Outcome::Held(shelf) = Outcome::Held(HeldReport {
        holdings: vec![unlocated()],
        findings: vec![operator(SERVER_SILENT), operator(UNREACHABLE)],
        ..HeldReport::default()
    })
    .for_household() else {
        panic!("a shelf stopped being a shelf");
    };
    let told = shelf.findings.into_iter().chain(
        shelf
            .holdings
            .into_iter()
            .filter_map(|held| held.at.unlocated),
    );
    for obstacle in told {
        assert_eq!(unhoused(&obstacle.said), None, "{obstacle}");
        assert_eq!(unhoused(&obstacle.remedy), None, "{obstacle}");
    }
}

/// An outcome that carries nothing to word again is left as it is.
#[test]
fn an_outcome_with_nothing_to_word_again_is_left_as_it_is() {
    let outcome = Outcome::Watched(crate::model::WatchedReport::default());
    assert_eq!(outcome.clone().for_household(), outcome);
}
