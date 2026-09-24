use super::{named, named_ones, unknown};
use crate::credential::{Held, Origin, Settled, State};

/// Two lines to look up between.
fn inventory() -> Vec<Held> {
    vec![
        line("qBittorrent web UI password", "QBITTORRENT_PASSWORD"),
        line("Sonarr API key", "SONARR_API_KEY"),
    ]
}

fn line(name: &str, setting: &str) -> Held {
    Held {
        name: name.to_owned(),
        setting: setting.to_owned(),
        consumers: Vec::new(),
        location: "somewhere".to_owned(),
        origin: Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: State::Active,
        fingerprint: None,
        advisory: None,
    }
}

#[test]
fn a_name_typed_exactly_as_it_is_printed_finds_its_line() {
    let held = inventory();
    let found = named(&held, "qBittorrent web UI password");

    assert_eq!(
        found.map(|one| one.setting.as_str()),
        Some("QBITTORRENT_PASSWORD")
    );
}

#[test]
fn the_setting_it_is_recorded_under_finds_it_too() {
    let held = inventory();
    let found = named(&held, "sonarr_api_key");

    assert_eq!(found.map(|one| one.name.as_str()), Some("Sonarr API key"));
}

#[test]
fn a_word_out_of_the_name_finds_it_without_the_rest_of_the_sentence() {
    let held = inventory();
    let found = named(&held, "  QBITTORRENT  ");

    assert_eq!(
        found.map(|one| one.name.as_str()),
        Some("qBittorrent web UI password")
    );
}

#[test]
fn nothing_typed_names_nothing_rather_than_the_first_line() {
    let held = inventory();
    assert!(named(&held, "   ").is_none());
}

#[test]
fn a_name_nothing_answers_to_finds_nothing() {
    let held = inventory();
    assert!(named(&held, "the wifi password").is_none());
}

#[test]
fn a_name_nothing_answers_to_is_told_what_would_have_been_accepted() {
    let held = inventory();
    let stopped = unknown("the wifi password", &named_ones(&held));

    assert!(stopped.kept_the_existing());
    assert!(stopped.consumers.is_empty());
    assert_eq!(
        stopped.settled,
        Settled::Unknown {
            known: vec![
                "qBittorrent web UI password".to_owned(),
                "Sonarr API key".to_owned(),
            ],
        }
    );
}
