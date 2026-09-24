use super::super::leak::Reach;
use super::super::Verdict;
use super::{
    held_from, not_asked_for, not_attempted, read_route, set_link, tunnel_device, verdict, Held,
};

#[test]
fn the_device_is_read_from_the_route_rather_than_assumed() {
    // OpenVPN and WireGuard name it differently and a fork could name it
    // anything; what they share is carrying the default route.
    assert_eq!(
        tunnel_device("default via 10.8.0.1 dev tun0 proto static").as_deref(),
        Some("tun0")
    );
    assert_eq!(
        tunnel_device("default dev wg0 scope link").as_deref(),
        Some("wg0")
    );
}

#[test]
fn a_route_naming_no_device_yields_none_rather_than_a_guess() {
    assert_eq!(tunnel_device(""), None);
    assert_eq!(tunnel_device("default via 10.8.0.1"), None);
    assert_eq!(tunnel_device("dev"), None, "the word with nothing after it");
}

#[test]
fn the_commands_read_and_move_the_link() {
    assert_eq!(read_route().last().map(String::as_str), Some("default"));
    assert_eq!(
        set_link("tun0", false).last().map(String::as_str),
        Some("down")
    );
    assert_eq!(
        set_link("tun0", true).last().map(String::as_str),
        Some("up")
    );
    assert!(set_link("wg0", true).contains(&"wg0".to_owned()));
}

/// A verdict as it reads, so a test can assert on its kind and its contents
/// without a `match` carrying arms this check never produces — and without a
/// `matches!` inside an `assert!`, whose failing branch nothing ever takes.
fn shown(said: &Verdict) -> String {
    format!("{said:?}")
}

#[test]
fn traffic_that_leaves_while_the_tunnel_is_down_is_the_leak_this_exists_to_find() {
    let leaked = held_from(&Reach::Address("203.0.113.7".to_owned()));
    assert_eq!(
        leaked,
        Held::No {
            seen: "203.0.113.7".to_owned()
        }
    );
    // And it is a failure, with the address the world saw attached — that
    // being the operator's own is the whole point.
    let said = shown(&verdict(&leaked));
    assert!(said.starts_with("Fail"), "{said}");
    assert!(
        said.contains("203.0.113.7"),
        "the address the world saw: {said}"
    );
}

#[test]
fn traffic_that_stops_is_the_guarantee_proven_rather_than_assumed() {
    assert_eq!(held_from(&Reach::Blocked), Held::Yes);
    assert_eq!(held_from(&Reach::Down), Held::Yes);
    assert!(shown(&verdict(&Held::Yes)).starts_with("Pass"));
}

#[test]
fn a_client_that_answered_unreadably_is_not_a_killswitch_that_held() {
    // The dangerous half of what used to be one state. `Blocked` meant both
    // "wget could not transfer" and "wget transferred and the body is not an
    // address" — and the second is a completed round trip to a public host
    // while the tunnel was down, reported as the guarantee proven.
    //
    // A captive portal, an ISP interception page, an echo answering JSON, and
    // any 4xx or 5xx from a server that did answer all produce it.
    let unreadable = held_from(&Reach::Unreadable);
    assert_ne!(
        unreadable,
        Held::Yes,
        "the client reached something, which is the opposite of traffic stopping"
    );
    let said = shown(&verdict(&unreadable));
    assert!(said.starts_with("Unverified"), "{said}");
    assert!(
        !said.starts_with("Pass"),
        "nothing about this proves the killswitch: {said}"
    );
}

#[test]
fn a_probe_that_could_not_run_accuses_nobody() {
    // Saying a stack leaks on the strength of a failed command would be a
    // fault report about the checker.
    let unknown = held_from(&Reach::Unknown);
    assert!(
        unknown != Held::Yes,
        "a failed probe is not a proven killswitch"
    );
    assert!(shown(&verdict(&unknown)).starts_with("Unverified"));
}

#[test]
fn a_tunnel_that_did_not_come_back_is_reported_as_the_fault_it_is() {
    // This check breaks something on purpose. Failing to put it back is not a
    // footnote — the operator is now without a tunnel and does not know it.
    let said = shown(&verdict(&Held::NotRestored));
    assert!(said.starts_with("Fail"), "{said}");
    assert!(said.contains("did not come back"), "{said}");
}

/// What it disturbs and for how long, both stated before anything is dropped.
///
/// The length is read from the budget that enforces it, so a check given longer
/// cannot go on promising the shorter answer.
#[test]
fn what_it_disturbs_is_said_with_how_long_it_disturbs_it_for() {
    let said = not_asked_for();
    assert!(
        said.contains("dropping the tunnel") && said.contains("interrupts transfers"),
        "it should say what it disturbs: {said}"
    );
    assert!(
        said.contains(&format!(
            "no longer than the {} seconds",
            super::DISTURBANCE.as_secs()
        )),
        "it should say how long for, in the seconds it is bounded to: {said}"
    );
}

#[test]
fn an_untested_killswitch_is_never_reported_as_working() {
    // The comfortable falsehood this feature exists to eliminate.
    let untested = Held::NotAttempted {
        reason: not_asked_for(),
    };
    let said = shown(&verdict(&untested));
    assert!(said.starts_with("Unverified"), "{said}");
    // And it says how to get a real answer, which the other unattempted
    // reasons cannot — they are about a stack that is not ready to be tested.
    assert!(
        said.contains("lemonfiber doctor --only vpn --disruptive"),
        "{said}"
    );
}

#[test]
fn a_reason_the_test_could_not_run_is_only_given_where_it_was_asked_for() {
    // Without the flag the operator gets the standing "you have not asked",
    // not a complaint about a stack nobody tried to test.
    let asked = not_attempted(true, "the tunnel container is not running");
    assert_eq!(
        asked,
        Held::NotAttempted {
            reason: "the tunnel container is not running".to_owned()
        }
    );
    assert_eq!(
        not_attempted(false, "the tunnel container is not running"),
        Held::NotAttempted {
            reason: not_asked_for()
        }
    );
}
