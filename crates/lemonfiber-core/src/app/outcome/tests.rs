use super::{Outcome, LISTED};
use crate::app::engine::LogsReport;
use crate::app::surfacing::Answer;
use crate::error::codes::read::NO_SUCH_READ;
use crate::model::kind::{self, Kind, OCTETS};
use crate::model::Octets;

fn pictured() -> Octets {
    Octets {
        media_type: "image/png",
        bytes: b"png".to_vec(),
    }
}

#[test]
fn bytes_are_handed_over_as_they_are() {
    assert_eq!(
        Outcome::Surfaced(Answer::Octets(pictured())).octets().ok(),
        Some(pictured())
    );
}

#[test]
fn a_report_is_not_handed_over_as_bytes() {
    let refused = Outcome::Logs(LogsReport { lines: 2 }).octets().err();

    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(NO_SUCH_READ)
    );
    assert_eq!(
        refused.and_then(|problem| problem.detail),
        Some("it answered under logs".to_owned())
    );
}

#[test]
fn every_outcome_answers_under_a_published_kind_but_bytes() {
    let mut carried: Vec<Kind> = Vec::new();
    Outcome::schemas(|kind, _| carried.push(kind));
    let unpublished: Vec<Kind> = LISTED
        .iter()
        .flatten()
        .chain(&carried)
        .copied()
        .chain([Outcome::Surfaced(Answer::Octets(pictured())).kind()])
        .filter(|answered| !kind::ALL.contains(answered))
        .collect();

    assert_eq!(unpublished, [OCTETS]);
    assert!(
        LISTED.iter().flatten().count() > 50,
        "the outcomes could not be read: {}",
        LISTED.len()
    );
}
