use lemonfiber_core::model::{Medium, Playback, PlayingReport};

use super::playing;

fn shown(report: &PlayingReport) -> String {
    playing(report).text()
}

fn episode() -> Playback {
    Playback {
        member_id: "a7f3".to_owned(),
        member: "Ada".to_owned(),
        title: "Pilot".to_owned(),
        series: Some("The Expanse".to_owned()),
        season: Some(1),
        episode: Some(2),
        medium: Medium::Series,
        paused: false,
        device: "TV".to_owned(),
    }
}

fn film() -> Playback {
    Playback {
        member_id: "b2e9".to_owned(),
        member: "Bram".to_owned(),
        title: "Arrival".to_owned(),
        series: None,
        season: None,
        episode: None,
        medium: Medium::Film,
        paused: true,
        device: "Phone".to_owned(),
    }
}

#[test]
fn every_session_is_a_line_saying_who_what_and_where() {
    let said = shown(&PlayingReport {
        sessions: vec![episode(), film()],
        available: true,
        ..PlayingReport::default()
    });
    assert!(said.starts_with("What is playing"), "{said}");
    assert!(
        said.contains("Ada — The Expanse, season 1 episode 2: Pilot (on TV)"),
        "{said}"
    );
    assert!(said.contains("Bram — Arrival (on Phone, paused)"), "{said}");
    assert!(said.contains("2 sessions playing"), "{said}");
}

#[test]
fn one_members_reading_names_them_and_counts_one() {
    let said = shown(&PlayingReport {
        member: "Ada".to_owned(),
        sessions: vec![episode()],
        available: true,
        ..PlayingReport::default()
    });
    assert!(said.starts_with("Ada — what is playing"), "{said}");
    assert!(said.contains("1 session playing"), "{said}");
}

#[test]
fn an_episode_without_numbers_still_names_its_series() {
    let said = shown(&PlayingReport {
        sessions: vec![Playback {
            season: None,
            ..episode()
        }],
        available: true,
        ..PlayingReport::default()
    });
    assert!(said.contains("Ada — The Expanse: Pilot (on TV)"), "{said}");
}

#[test]
fn nobody_watching_and_an_unread_server_read_differently() {
    let quiet = shown(&PlayingReport {
        available: true,
        ..PlayingReport::default()
    });
    assert!(quiet.contains("Nothing is playing"), "{quiet}");

    let unread = shown(&PlayingReport {
        findings: vec!["the media server would not say".to_owned()],
        ..PlayingReport::default()
    });
    assert!(!unread.contains("Nothing is playing"), "{unread}");
    assert!(!unread.contains("playing\n  0"), "{unread}");
    assert!(
        unread.contains("! the media server would not say"),
        "{unread}"
    );
}
