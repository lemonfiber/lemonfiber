//! Proving the killswitch by breaking the tunnel and watching the traffic stop.
//!
//! Every other VPN check observes a healthy stack and infers. This one does not
//! infer: it takes the tunnel away and asks the download client whether it can
//! still reach the internet. A stack whose killswitch works answers no. A stack
//! whose killswitch is a comfortable assumption answers with a public address —
//! the operator's own — and that is the moment worth finding out, rather than
//! after months of torrenting in the open.
//!
//! It is disruptive by nature and gated behind the operator asking for it. What
//! it disturbs it puts back: the tunnel is restored whatever the probe found,
//! and a restoration that cannot be confirmed is reported as the fault it is,
//! never quietly left.
//!
//! The device is discovered rather than named. `gluetun` runs `OpenVPN` over `tun0`
//! and `WireGuard` over `wg0`, and a fork could run neither; what they all share
//! is that the tunnel carries the default route, so that is what is read and
//! that is what is dropped.

use crate::error::{Code, Remedy};

use super::leak::Reach;
use super::Verdict;

/// The code a stack whose traffic survives its tunnel earns.
pub const KILLSWITCH_LEAKS: Code = Code::new("VPN-5");

/// The code a stack whose tunnel could not be put back earns.
pub const TUNNEL_NOT_RESTORED: Code = Code::new("VPN-6");

/// What the test established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Held {
    /// The tunnel went down and the client's traffic stopped with it. The
    /// guarantee is real, and now proven rather than assumed.
    Yes,
    /// The tunnel went down and the client still reached the internet, from this
    /// address. This is the leak the whole check exists to find.
    No {
        /// The address the world saw while the tunnel was down — the operator's
        /// own, which is the point.
        seen: String,
    },
    /// Nothing was disturbed, and nothing was proven.
    NotAttempted {
        /// Why it could not be run.
        reason: String,
    },
    /// It was disturbed, and putting it back could not be confirmed.
    NotRestored,
}

/// The command that reads which device carries the default route.
pub(super) fn read_route() -> Vec<String> {
    vec![
        "ip".to_owned(),
        "route".to_owned(),
        "show".to_owned(),
        "default".to_owned(),
    ]
}

/// The command that takes `device` down, and the one that puts it back.
pub(super) fn set_link(device: &str, up: bool) -> Vec<String> {
    vec![
        "ip".to_owned(),
        "link".to_owned(),
        "set".to_owned(),
        device.to_owned(),
        if up { "up" } else { "down" }.to_owned(),
    ]
}

/// The device carrying the default route, as `ip route show default` reports it.
///
/// The line reads `default via 10.8.0.1 dev tun0 ...`; the device is the word
/// after `dev`. Nothing is assumed about its name — that is the whole reason
/// this is read rather than hard-coded.
pub(super) fn tunnel_device(route: &str) -> Option<String> {
    route
        .split_whitespace()
        .skip_while(|word| *word != "dev")
        .nth(1)
        .map(str::to_owned)
}

/// What the client's answer means once the tunnel is down.
///
/// A client that cannot be reached at all counts as held: the test is whether
/// traffic *left*, and an unrunnable probe did not leave. Saying otherwise would
/// accuse a working stack of leaking on the strength of a failed command.
pub(super) fn held_from(reach: &Reach) -> Held {
    match reach {
        Reach::Address(seen) => Held::No { seen: seen.clone() },
        Reach::Blocked | Reach::Down => Held::Yes,
        Reach::Unknown => Held::NotAttempted {
            reason: "the download client could not be asked while the tunnel was down".to_owned(),
        },
    }
}

/// Nothing was disturbed and nothing was proven, said either as the reason it
/// could not run or as the standing "you have not asked for this".
pub(super) fn not_attempted(disruptive: bool, reason: &str) -> Held {
    Held::NotAttempted {
        reason: if disruptive {
            reason.to_owned()
        } else {
            NOT_ASKED_FOR.to_owned()
        },
    }
}

/// The finding this test earns.
pub(super) fn verdict(held: &Held) -> Verdict {
    match held {
        Held::Yes => Verdict::Pass {
            note: Some("traffic stopped when the tunnel was dropped".to_owned()),
        },
        Held::No { seen } => Verdict::Fail(
            crate::error::Problem::new(
                KILLSWITCH_LEAKS,
                crate::error::Severity::Error,
                "your traffic survives the tunnel going down",
                "The tunnel was dropped and the download client still reached the internet, \
                 which means every torrent would continue in the open the moment the VPN \
                 fails — and a VPN that never fails is not a thing.",
                Remedy::new(
                    "Enable the tunnel container's own killswitch — for gluetun, \
                             `FIREWALL=on`, which is its default",
                ),
            )
            .or_try(Remedy::new(
                "Or confirm nothing else gives the client a second route out",
            ))
            .in_state(crate::error::State::Guided)
            .with_detail(format!("the world saw {seen} while the tunnel was down")),
        ),
        Held::NotAttempted { reason } if reason == NOT_ASKED_FOR => Verdict::Unverified {
            reason: reason.clone(),
            remedy: Remedy::new("Run the disruptive check when transfers can be interrupted")
                .with_detail("lemonfiber doctor --only vpn --disruptive"),
        },
        Held::NotAttempted { reason } => Verdict::Unverified {
            reason: reason.clone(),
            remedy: Remedy::new(
                "Confirm the tunnel and the client are both up, then run this again",
            ),
        },
        Held::NotRestored => Verdict::Fail(
            crate::error::Problem::new(
                TUNNEL_NOT_RESTORED,
                crate::error::Severity::Error,
                "the tunnel was dropped for the test and did not come back",
                "This check takes the tunnel away on purpose and puts it back. Putting it \
                 back could not be confirmed, so the stack is left without one — and \
                 whether traffic is flowing outside it is exactly what is now unknown.",
                Remedy::new("Restart the tunnel container now"),
            )
            .in_state(crate::error::State::Guided),
        ),
    }
}

/// What the killswitch finding says when the operator has not asked for the
/// disruptive checks.
///
/// It is never a pass. An untested fail-closed guarantee reported as working would
/// be exactly the comfortable falsehood this feature exists to eliminate.
pub(super) const NOT_ASKED_FOR: &str =
    "the killswitch has not been tested; proving it works means dropping the tunnel and \
     confirming traffic stops, which interrupts transfers";

#[cfg(test)]
mod tests {
    use super::super::leak::Reach;
    use super::super::Verdict;
    use super::{
        held_from, not_attempted, read_route, set_link, tunnel_device, verdict, Held, NOT_ASKED_FOR,
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

    #[test]
    fn an_untested_killswitch_is_never_reported_as_working() {
        // The comfortable falsehood this feature exists to eliminate.
        let untested = Held::NotAttempted {
            reason: NOT_ASKED_FOR.to_owned(),
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
                reason: NOT_ASKED_FOR.to_owned()
            }
        );
    }
}
