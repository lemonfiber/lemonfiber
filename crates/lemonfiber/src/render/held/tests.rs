use lemonfiber_core::model::{Held, HeldReport, Medium};
use lemonfiber_core::ports::service::{Holds, Located};

use super::held;

fn one(title: &str, year: Option<u16>, medium: Medium) -> Held {
    Held {
        id: title.to_owned(),
        title: title.to_owned(),
        year,
        medium,
        at: Located::default(),
        holds: Holds::default(),
    }
}

fn shown(report: &HeldReport) -> String {
    held(report).text()
}

fn a_shelf() -> HeldReport {
    HeldReport {
        rehearsed: false,
        member: "Ada".to_owned(),
        id: "a7f3".to_owned(),
        holdings: vec![
            one("A Film", Some(1994), Medium::Film),
            one("A Series", None, Medium::Series),
            one("An Album", Some(1973), Medium::Other),
        ],
        available: true,
        findings: Vec::new(),
    }
}

#[test]
fn every_kind_is_said_in_words_and_a_missing_year_is_left_out() {
    let said = shown(&a_shelf());
    assert!(said.contains("A Film (1994, film)"), "{said}");
    assert!(said.contains("A Series (series)"), "{said}");
    assert!(said.contains("An Album (1973, something else)"), "{said}");
    assert!(said.contains("3 to watch — 1 film, 1 series"), "{said}");
}

/// The pair this screen exists to keep apart. An empty shelf says it is empty; a
/// shelf that could not be read says that instead, and neither is ever printed as
/// the other.
#[test]
fn an_empty_shelf_and_an_unread_one_do_not_read_alike() {
    let empty = shown(&HeldReport {
        member: "Ada".to_owned(),
        available: true,
        ..HeldReport::default()
    });
    assert!(empty.contains("Nothing this member can watch"), "{empty}");

    let unread = shown(&HeldReport {
        member: "Ada".to_owned(),
        findings: vec!["the media server would not say".to_owned()],
        ..HeldReport::default()
    });
    assert!(
        !unread.contains("Nothing this member can watch"),
        "{unread}"
    );
    assert!(
        unread.contains("! the media server would not say"),
        "{unread}"
    );
}

/// Two films are counted in the plural, and an episode is said as one.
#[test]
fn films_are_counted_in_the_plural_and_an_episode_is_named() {
    let said = shown(&HeldReport {
        member: "Ada".to_owned(),
        holdings: vec![
            one("Heat", Some(1995), Medium::Film),
            one("Ronin", Some(1998), Medium::Film),
            one("Pilot", None, Medium::Episode),
        ],
        available: true,
        ..HeldReport::default()
    });
    assert!(said.contains("Pilot (episode)"), "{said}");
    assert!(said.contains("3 to watch — 2 films, 0 series"), "{said}");
}
