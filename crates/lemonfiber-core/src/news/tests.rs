use super::{News, NewsKind, NEWEST};
use crate::asking::Policy;
use crate::changelog::Record;
use crate::error::Severity;
use crate::health::Affected;
use crate::model::{HouseholdMember, HouseholdReport, MemberRequest, TitleKind, TitleUnit};

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

/// A request named `id`, asked for at the hour `hour` of one day where one is given.
fn asked(id: &str, hour: Option<u32>, title: Option<&str>) -> MemberRequest {
    MemberRequest {
        year: None,
        arrived: None,
        shelf_id: None,
        id: id.to_owned(),
        asked: hour.map(|hour| format!("2026-11-08T{hour:02}:00:00.000Z")),
        title: title.map(str::to_owned),
        media: None,
        kind: None,
        unit: None,
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
        acknowledged: false,
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
fn a_request_is_newer_by_when_it_was_asked_whoever_keeps_it_and_whoever_asked() {
    let house = household(vec![
        member(
            "Anna",
            vec![
                asked("3", Some(3), Some("Dune")),
                asked("music-1", Some(12), Some("Blue")),
                asked("9", None, None),
            ],
        ),
        member("Bram", vec![asked("7", Some(7), Some("Bluey"))]),
    ]);

    let news = News::of(None, Some(&house), &[]);

    let named: Vec<(&str, &str)> = news
        .requests
        .iter()
        .map(|one| (one.request.as_str(), one.by.as_str()))
        .collect();
    assert_eq!(
        named,
        vec![
            ("music-1", "Anna"),
            ("7", "Bram"),
            ("3", "Anna"),
            ("9", "Anna")
        ]
    );
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
        (1..=15)
            .map(|id| asked(&id.to_string(), Some(id), None))
            .collect(),
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
    assert_eq!(newest.requests.last().map(String::as_str), Some("6"));
}

#[test]
fn a_request_in_the_news_carries_the_kind_and_unit_the_household_list_names() {
    let house = household(vec![member(
        "Anna",
        vec![
            MemberRequest {
                kind: Some(TitleKind::Books),
                unit: Some(TitleUnit::Author),
                ..asked("books-2", Some(9), Some("Ursula K. Le Guin"))
            },
            MemberRequest {
                kind: Some(TitleKind::Tv),
                ..asked("4", Some(8), Some("The Expanse"))
            },
            asked("5", Some(7), None),
        ],
    )]);

    let news = News::of(None, Some(&house), &[]);

    let said: Vec<(&str, Option<TitleKind>, Option<TitleUnit>)> = news
        .requests
        .iter()
        .map(|one| (one.request.as_str(), one.kind, one.unit))
        .collect();
    assert_eq!(
        said,
        vec![
            ("books-2", Some(TitleKind::Books), Some(TitleUnit::Author)),
            ("4", Some(TitleKind::Tv), None),
            ("5", None, None),
        ]
    );
    let written: Vec<serde_json::Value> = news
        .requests
        .iter()
        .filter_map(|one| serde_json::to_value(one).ok())
        .collect();
    assert_eq!(
        written
            .first()
            .map(|one| (one.get("kind").cloned(), one.get("unit").cloned())),
        Some((
            Some(serde_json::json!("books")),
            Some(serde_json::json!("author"))
        ))
    );
    assert!(
        written.get(1).is_some_and(|one| one.get("unit").is_none()),
        "{written:?}"
    );
    assert!(
        written
            .get(2)
            .is_some_and(|one| one.get("kind").is_none() && one.get("unit").is_none()),
        "{written:?}"
    );
}
