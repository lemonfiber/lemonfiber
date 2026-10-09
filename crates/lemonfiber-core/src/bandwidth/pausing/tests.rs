use super::{Paused, Pauses, Pausing};
use crate::bandwidth::Pulling;

/// One client, read back as `now`.
fn read_back(now: Option<Pulling>) -> Paused {
    Paused {
        client: "qbittorrent".to_owned(),
        was: Some(Pulling::Fetching),
        now,
        unreached: None,
    }
}

/// A report on these clients.
fn report(asked: Pausing, clients: Vec<Paused>, rehearsed: bool) -> Pauses {
    Pauses {
        asked,
        clients,
        caution: None,
        rehearsed,
        offer: String::new(),
    }
}

#[test]
fn a_pause_wants_every_client_stopped_and_a_resume_wants_it_fetching() {
    assert_eq!(Pausing::Pause.wanted(), Pulling::Stopped);
    assert_eq!(Pausing::Resume.wanted(), Pulling::Fetching);
    assert_eq!(Pausing::Pause.word(), "pause");
    assert_eq!(Pausing::Resume.word(), "resume");
}

#[test]
fn a_request_is_whole_only_where_every_client_read_back_what_it_wanted() {
    let kept = report(
        Pausing::Pause,
        vec![read_back(Some(Pulling::Stopped))],
        false,
    );
    assert!(kept.whole());
    let ignored = report(
        Pausing::Pause,
        vec![
            read_back(Some(Pulling::Stopped)),
            read_back(Some(Pulling::Fetching)),
        ],
        false,
    );
    assert!(!ignored.whole());
    // A client nobody reached read nothing back, which is not where it was wanted.
    let unreached = report(Pausing::Resume, vec![read_back(None)], false);
    assert!(!unreached.whole());
}

#[test]
fn a_rehearsal_asked_nothing_so_it_has_nothing_to_fall_short_of() {
    assert!(report(Pausing::Pause, vec![read_back(None)], true).whole());
}

#[test]
fn the_request_is_spelled_as_one_word_on_the_wire() {
    assert_eq!(
        serde_json::to_value(Pausing::Resume).ok(),
        Some(serde_json::json!("resume"))
    );
}
