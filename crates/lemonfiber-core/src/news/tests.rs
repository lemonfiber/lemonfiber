use super::{News, NewsKind, NEWEST};
use crate::asking::Policy;
use crate::changelog::Record;
use crate::error::Severity;
use crate::health::Affected;
use crate::model::{HouseholdMember, HouseholdReport, MemberRequest};

/// A record holding three releases, a file each, as the pipeline writes them.
fn record() -> Option<Record> {
    Record::read(&[
        (
            "0.17.1.json",
            r#"{"version":"0.17.1","tag":"v0.17.1","delivers":"Fixes","user_facing":true,"groups":[]}"#,
        ),
        (
            "0.17.0.json",
            r#"{"version":"0.17.0","tag":"v0.17.0","delivers":"Plugins, part two","user_facing":true,"groups":[]}"#,
        ),
        (
            "0.16.0.json",
            r#"{"version":"0.16.0","tag":"v0.16.0","user_facing":true,"groups":[]}"#,
        ),
    ])
}

fn asked(id: i64, title: Option<&str>) -> MemberRequest {
    MemberRequest {
        year: None,
        arrived: None,
        shelf_id: None,
        id,
        title: title.map(str::to_owned),
        media: None,
        state: None,
        waiting_days: None,
        estimate: None,
        refused: None,
    }
}

fn member(name: &str, requests: Vec<MemberRequest>) -> HouseholdMember {
    HouseholdMember {
        name: name.to_owned(),
        requests,
        ..HouseholdMember::default()
    }
}

/// A household the media server and the request service both answered for.
fn household(members: Vec<HouseholdMember>) -> HouseholdReport {
    HouseholdReport {
        members,
        available: true,
        policy: Some(Policy::Trusted),
        ..HouseholdReport::default()
    }
}

fn wrong(check: &str, onset: &str) -> Affected {
    Affected {
        check: check.to_owned(),
        onset: onset.to_owned(),
        severity: Severity::Error,
        summary: format!("{check} is wrong"),
        meaning: String::new(),
        remedies: Vec::new(),
        downstream: Vec::new(),
        exit: None,
    }
}

#[test]
fn a_release_is_newer_by_its_place_in_the_record() {
    let news = News::of(record().as_ref(), None, &[]);

    let versions: Vec<&str> = news
        .updates
        .iter()
        .map(|one| one.version.as_str())
        .collect();
    assert_eq!(versions, vec!["0.17.1", "0.17.0", "0.16.0"]);
    assert_eq!(
        news.updates.get(1).and_then(|one| one.delivers.as_deref()),
        Some("Plugins, part two")
    );
}

#[test]
fn a_request_is_newer_by_its_number_whoever_asked() {
    let house = household(vec![
        member("Anna", vec![asked(3, Some("Dune")), asked(12, None)]),
        member("Bram", vec![asked(7, Some("Bluey"))]),
    ]);

    let news = News::of(None, Some(&house), &[]);

    let numbers: Vec<(i64, &str)> = news
        .requests
        .iter()
        .map(|one| (one.number, one.by.as_str()))
        .collect();
    assert_eq!(numbers, vec![(12, "Anna"), (7, "Bram"), (3, "Anna")]);
}

#[test]
fn a_problem_is_newer_by_when_it_went_wrong_and_by_check_within_a_moment() {
    let affected = [
        wrong("vpn.egress", "1000"),
        wrong("service.sonarr", "3000"),
        wrong("service.radarr", "3000"),
        wrong("storage.space", "written by hand"),
    ];

    let news = News::of(None, None, &affected);

    let checks: Vec<&str> = news.problems.iter().map(|one| one.check.as_str()).collect();
    assert_eq!(
        checks,
        vec![
            "service.radarr",
            "service.sonarr",
            "vpn.egress",
            "storage.space"
        ]
    );
}

#[test]
fn a_kind_that_could_not_be_read_is_named_rather_than_left_empty() {
    // An empty list a surface took for everything there is would have it mark every
    // request as new the first time the request service answered again.
    let nothing_asked = HouseholdReport {
        policy: None,
        ..household(vec![member("Anna", Vec::new())])
    };

    assert_eq!(
        News::of(None, Some(&nothing_asked), &[]).unread,
        vec![NewsKind::Updates, NewsKind::Requests]
    );
    assert_eq!(
        News::of(record().as_ref(), Some(&household(Vec::new())), &[]).unread,
        Vec::new()
    );
}

#[test]
fn the_newest_of_each_kind_is_the_first_ten_by_what_names_them() {
    let affected: Vec<Affected> = (0..15)
        .map(|at| wrong(&format!("check.{at:02}"), &(1000 + at).to_string()))
        .collect();
    let house = household(vec![member(
        "Anna",
        (1..=15).map(|id| asked(id, None)).collect(),
    )]);

    let newest = News::of(record().as_ref(), Some(&house), &affected).newest();

    assert_eq!(newest.problems.len(), NEWEST);
    assert_eq!(newest.requests.len(), NEWEST);
    assert_eq!(
        newest.updates,
        vec!["0.17.1", "0.17.0", "0.16.0"],
        "a kind with fewer keeps all of them"
    );
    assert_eq!(
        newest
            .problems
            .first()
            .map(|one| (one.check.as_str(), one.onset.as_str())),
        Some(("check.14", "1014"))
    );
    assert_eq!(newest.requests.last(), Some(&6));
}
