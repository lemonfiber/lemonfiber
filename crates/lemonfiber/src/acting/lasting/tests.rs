use super::{all, every, Going, Lasting, KEY, LET_GO};
use lemonfiber::reaching::ACTS;
use lemonfiber_api::actions::OFFERED as WEB;
use lemonfiber_api::jobs::{leased, Lease};
use lemonfiber_core::app::Command;

/// One of these naming an action no surface offers, for the path that reports a
/// translation which reached no command.
pub(crate) static NOTHING_ANSWERS: Lasting = Lasting {
    name: "a walk nothing answers",
    about: "for the refusal a translation that reaches no command produces",
    action: "not an action any surface offers",
    asks: "Walk through",
    going: Going::Walking("What to look for"),
};

/// What one action is called on the list, for the screen's own tests, which move
/// to an entry by the name an operator would read rather than by number.
pub(crate) fn listed(action: &str) -> String {
    every()
        .filter(|lasting| lasting.action == action)
        .map(|lasting| lasting.name)
        .collect()
}

/// The whole point of naming the action rather than assembling a command here:
/// what this screen starts has to be something another surface already offers, or
/// the requirement it is being built for is defeated by the thing built for it.
#[test]
fn every_request_this_list_starts_is_one_the_other_surfaces_offer() {
    let missing: Vec<&str> = every()
        .map(|lasting| lasting.action)
        .filter(|action| !WEB.contains(action))
        .collect();

    assert!(missing.is_empty(), "{missing:?}");
}

/// The key this list opens on is not one the screen already answers, or the thing
/// it already did stops happening and nothing says so.
#[test]
fn the_key_that_opens_them_is_not_one_the_screen_already_answers() {
    for taken in [
        'q',
        'r',
        '?',
        'y',
        crate::acting::question::KEY,
        crate::acting::errand::KEY,
        crate::acting::surface::KEY,
    ] {
        assert_ne!(KEY, taken, "{taken:?} was already spoken for");
    }
    for offer in crate::acting::offer::OFFERED {
        assert_ne!(KEY, offer.key, "{:?} was already spoken for", offer.key);
    }
}

/// Each is named once and says what it does, since the line under the name is the
/// whole of what somebody chooses between them on.
#[test]
fn each_is_named_once_and_says_what_it_does() {
    for lasting in every() {
        let same = every().filter(|other| other.name == lasting.name).count();
        assert_eq!(same, 1, "more than one is called {}", lasting.name);
        assert!(!lasting.about.is_empty(), "{}", lasting.name);
        assert!(!lasting.asks.is_empty(), "{}", lasting.name);
    }
}

/// The list opens on the walk and holds them all.
#[test]
fn the_list_opens_on_the_walk_and_holds_them_all() {
    let (first, rest) = all();

    assert_eq!(first.action, "walkthrough");
    assert!(matches!(first.going, Going::Walking(_)));
    assert_eq!(rest.len() + 1, every().count());
}

/// The one this screen offers to end is the one the web holds on a lease, asked
/// of the web's own table rather than decided twice. A second list would come to
/// disagree, and where it disagreed the screen would either offer to end work
/// that was going to succeed or leave an operator with no way to end work that
/// never will.
#[test]
fn the_only_one_offered_an_end_is_the_one_the_web_leases() {
    let guard = Command::Watch {
        forms: vec!["media".to_owned()],
    };
    let walk = Command::Walkthrough { item: None };

    assert_eq!(leased(&guard), Lease::WhileAsked);
    assert_eq!(leased(&walk), Lease::Held);
    assert_eq!(listed("watch"), "a guard on the data location");
}

/// Letting a guard go says that it stopped guarding and stopped nothing else.
#[test]
fn letting_a_guard_go_says_the_services_were_left_alone() {
    assert!(LET_GO.contains("as they were"));
}

/// The one thing this list is checked against from outside the binary, for the
/// half of it that names an action. An entry here with no entry there leaves the
/// parity table's terminal column claiming less than the screen does.
#[test]
fn every_action_this_list_names_is_published_for_the_parity_table() {
    let published: Vec<&str> = ACTS.iter().map(|reach| reach.through).collect();

    for lasting in every() {
        assert!(
            published.contains(&lasting.action),
            "{} reaches {} and no entry says so",
            lasting.name,
            lasting.action
        );
    }
}
