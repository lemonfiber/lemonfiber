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
        unlocated: Some(lemonfiber_core::error::household::Obstacle::new(
            lemonfiber_core::error::codes::play::SERVER_SILENT,
            "no address",
        )),
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
        findings: vec![lemonfiber_core::error::household::Obstacle::new(
            lemonfiber_core::error::codes::play::SERVER_SILENT,
            "unread",
        )],
        ..PartWayReport::default()
    };
    let text = part_way(&report).text();
    assert!(text.contains("  ! unread"), "{text}");
    assert!(!text.contains("Nothing part-way through"), "{text}");
}

/// A channel is a line with its number, its name, what is on and its certificate where
/// each is known, and its id; no channel at all is said as the answer.
#[test]
fn channels_are_listed_with_what_is_on_and_none_is_said_as_the_answer() {
    use lemonfiber_core::model::ChannelsReport;
    use lemonfiber_core::ports::service::{Channel, Programme};
    let mut report = ChannelsReport {
        member: "Ada".to_owned(),
        id: "a7f3".to_owned(),
        channels: vec![
            Channel {
                id: "c1".to_owned(),
                name: "Kids".to_owned(),
                number: Some("7".to_owned()),
                now: Some(Programme {
                    title: "Cartoons".to_owned(),
                    certificate: Some("U".to_owned()),
                    ends: None,
                }),
            },
            Channel {
                id: "c2".to_owned(),
                name: "News".to_owned(),
                number: None,
                now: Some(Programme {
                    title: "Headlines".to_owned(),
                    certificate: None,
                    ends: None,
                }),
            },
            Channel {
                id: "c3".to_owned(),
                name: "Quiet".to_owned(),
                number: None,
                now: None,
            },
        ],
        rehearsed: false,
    };
    let text = super::channels(&report).text();
    assert!(text.contains("Ada — live TV"), "{text}");
    assert!(text.contains("  7 Kids — Cartoons (U)  [c1]"), "{text}");
    assert!(text.contains("  News — Headlines  [c2]"), "{text}");
    assert!(text.contains("  Quiet  [c3]"), "{text}");
    report.channels.clear();
    assert!(super::channels(&report)
        .text()
        .contains("No channel this member may watch now"));
}

/// A live stream opened says where each path is served, the door it presents and a code
/// for each; one closed, rehearsed or unlocated says that instead.
#[test]
fn a_live_stream_says_where_it_is_served_or_why_it_is_not() {
    use lemonfiber_core::model::TunedReport;
    let opened = TunedReport {
        channel: "c1".to_owned(),
        member: "Ada".to_owned(),
        stream: Some("ls1".to_owned()),
        direct: Some("https://h:8920/live/ls1/stream.ts?pass=p".to_owned()),
        transcoded: Some("https://h:8920/videos/c1/live.m3u8?pass=p".to_owned()),
        fingerprint: Some("ab12".to_owned()),
        lapses: Some(1_800_000_000),
        unlocated: None,
        rehearsed: false,
    };
    let text = super::tuned(&opened).text();
    assert!(text.contains("Channel c1 is open for Ada as ls1"), "{text}");
    assert!(text.contains("the door presents ab12"), "{text}");
    assert!(text.contains("As the tuner sends it:"), "{text}");
    assert!(
        text.contains("  https://h:8920/videos/c1/live.m3u8?pass=p"),
        "{text}"
    );
    assert_eq!(text.matches("The same, for a camera:").count(), 2, "{text}");

    let closed = TunedReport {
        channel: "c1".to_owned(),
        member: "Ada".to_owned(),
        ..TunedReport::default()
    };
    assert!(super::tuned(&closed)
        .text()
        .contains("Channel c1 is closed for Ada."));
    let rehearsed = TunedReport {
        rehearsed: true,
        ..closed.clone()
    };
    assert!(super::tuned(&rehearsed)
        .text()
        .contains("rehearsed, so nothing was opened"));
    let unlocated = TunedReport {
        unlocated: Some("no door".to_owned()),
        ..closed
    };
    assert_eq!(super::tuned(&unlocated).text().trim(), "! no door");
}

/// A title playing in a browser says where, on what pass, with a code; a rehearsal says
/// it would play, and an unlocated one why it does not.
#[test]
fn a_browser_play_says_where_it_plays_or_why_it_does_not() {
    use lemonfiber_core::model::PlayReport;
    let rehearsed = PlayReport {
        id: "f1".to_owned(),
        member: "Ada".to_owned(),
        ..PlayReport::default()
    };
    assert!(super::play(&rehearsed)
        .text()
        .contains("f1 would play in a browser as Ada."));
    let playing = PlayReport {
        stream_from: Some("https://door.example/videos/f1/master.m3u8?pass=p".to_owned()),
        lapses: Some(1_800_000_000),
        ..rehearsed.clone()
    };
    let text = super::play(&playing).text();
    assert!(
        text.contains("f1 plays in a browser as Ada, on a pass lapsing at"),
        "{text}"
    );
    assert!(text.contains("The same, for a camera:"), "{text}");
    let unlocated = PlayReport {
        unlocated: Some("no route".to_owned()),
        ..rehearsed
    };
    assert_eq!(super::play(&unlocated).text().trim(), "! no route");
}
