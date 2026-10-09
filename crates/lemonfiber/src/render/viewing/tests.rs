use lemonfiber_core::model::{Episode, Located, Pinned, Season};
use lemonfiber_core::model::{GrantReport, Held, Medium, PartWay, PartWayReport, TitleReport};
use lemonfiber_core::model::{Title, WatchedReport};

use super::{clock, grant, part_way, title, watched};

fn held(at: Located) -> Held {
    Held {
        id: "f1".to_owned(),
        title: "Heat".to_owned(),
        year: Some(1995),
        medium: Medium::Film,
        at,
    }
}

fn a_title(at: Located) -> TitleReport {
    TitleReport {
        member: "ana".to_owned(),
        id: "a7".to_owned(),
        title: Some(Title {
            held: held(at),
            overview: Some("A heist.".to_owned()),
            minutes: Some(170),
            genres: vec!["Crime".to_owned()],
            certificate: Some("16".to_owned()),
            released: None,
            seasons: Vec::new(),
        }),
        rehearsed: false,
    }
}

#[test]
fn a_located_title_says_where_it_plays_and_what_the_door_presents() {
    let text = title(&a_title(Located {
        stream_from: Some("https://h:8920/Videos/f1/master.m3u8".to_owned()),
        door: Some(Pinned {
            fingerprint: "ab12".to_owned(),
        }),
        ..Located::default()
    }))
    .text();
    assert!(text.contains("Heat (1995, film)"), "{text}");
    assert!(text.contains("170 minutes · 16 · Crime"), "{text}");
    assert!(
        text.contains("plays from https://h:8920/Videos/f1/master.m3u8"),
        "{text}"
    );
    assert!(text.contains("the door presents ab12"), "{text}");
}

#[test]
fn an_unlocated_title_says_why() {
    let text = title(&a_title(Located {
        unlocated: Some("no address".to_owned()),
        ..Located::default()
    }))
    .text();
    assert!(text.contains("! no address"), "{text}");
    assert!(!text.contains("plays from"), "{text}");
}

#[test]
fn part_way_says_how_far_of_how_long() {
    let report = PartWayReport {
        member: "ana".to_owned(),
        part_way: vec![PartWay {
            held: held(Located::default()),
            position: 3725,
            length: Some(6000),
        }],
        available: true,
        ..PartWayReport::default()
    };
    let text = part_way(&report).text();
    assert!(text.contains("at 1:02:05 of 1:40:00"), "{text}");
}

#[test]
fn nothing_part_way_is_said_as_the_answer() {
    let report = PartWayReport {
        available: true,
        ..PartWayReport::default()
    };
    assert!(part_way(&report)
        .text()
        .contains("Nothing part-way through"));
}

#[test]
fn a_grant_and_a_rehearsal_of_one_read_differently() {
    let mut report = GrantReport {
        member: "ana".to_owned(),
        granted: true,
        token: Some("fe".repeat(16)),
        lasts_until: "2026-11-08".to_owned(),
        rehearsed: false,
    };
    let said = grant(&report).text();
    assert!(said.contains("now plays as ana, until 2026-11-08"));
    assert!(!said.contains(&"fe".repeat(16)));
    report.granted = false;
    assert!(grant(&report).text().contains("would play as ana"));
}

#[test]
fn progress_is_said_as_a_finish_or_a_time() {
    let mut report = WatchedReport {
        id: "f1".to_owned(),
        position: 90,
        ended: false,
        rehearsed: false,
    };
    assert!(watched(&report).text().contains("Recorded at 1:30."));
    report.ended = true;
    assert!(watched(&report).text().contains("finished"));
    assert_eq!(clock(59), "0:59");
}

#[test]
fn a_title_off_the_shelf_is_said_as_such() {
    let report = TitleReport {
        title: None,
        ..a_title(Located::default())
    };
    assert!(title(&report)
        .text()
        .contains("No such title on this shelf."));
}

/// A series lists each season, and each episode by its number where it has one.
#[test]
fn a_series_lists_its_seasons_and_numbered_episodes() {
    let episode = |name: &str, number: Option<u32>| Episode {
        held: Held {
            title: name.to_owned(),
            medium: Medium::Episode,
            ..held(Located::default())
        },
        number,
        overview: None,
        minutes: None,
    };
    let mut report = a_title(Located::default());
    if let Some(title) = report.title.as_mut() {
        title.seasons = vec![Season {
            id: "s1".to_owned(),
            name: "Season 1".to_owned(),
            number: Some(1),
            episodes: vec![episode("Pilot", Some(1)), episode("Special", None)],
        }];
    }
    let text = title(&report).text();
    assert!(text.contains("  Season 1"), "{text}");
    assert!(text.contains("    1. Pilot"), "{text}");
    assert!(text.contains("    Special"), "{text}");
}

#[test]
fn what_could_not_be_read_part_way_is_said() {
    let report = PartWayReport {
        member: "ana".to_owned(),
        findings: vec!["unread".to_owned()],
        ..PartWayReport::default()
    };
    let text = part_way(&report).text();
    assert!(text.contains("  ! unread"), "{text}");
    assert!(!text.contains("Nothing part-way through"), "{text}");
}
