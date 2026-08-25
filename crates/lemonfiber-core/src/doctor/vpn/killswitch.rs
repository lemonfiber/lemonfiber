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
//! "Whatever the probe found" is not the only way this ends. A check is abandoned at
//! its budget by dropping the future it runs in, and a dropped future runs no further
//! code — so a restore reached only by returning would be no restore at all on the one
//! run that most needed it. Nothing is dropped unless the whole disturbance fits in
//! what is left of the budget, and inside it the probe and the restore are each
//! bounded, so the moment the budget expires is never a moment the tunnel is down.
//!
//! The device is discovered rather than named. `gluetun` runs `OpenVPN` over `tun0`
//! and `WireGuard` over `wg0`, and a fork could run neither; what they all share
//! is that the tunnel carries the default route, so that is what is read and
//! that is what is dropped.

use std::time::Duration;

use tokio::time::{timeout, Instant};

use crate::error::{Code, Remedy};

use super::leak::Reach;
use super::probe::running;
use super::Verdict;
use crate::ports::docker::Container;

/// The code a stack whose traffic survives its tunnel earns.
pub const KILLSWITCH_LEAKS: Code = Code::new("VPN-5");

/// The code a stack whose tunnel could not be put back earns.
pub const TUNNEL_NOT_RESTORED: Code = Code::new("VPN-6");

/// Seconds the download client is given to answer while the tunnel is down.
const PROBE_SECONDS: u64 = 5;

/// Seconds putting the tunnel back is given before nobody is waiting for it any more.
const RESTORE_SECONDS: u64 = 5;

/// How long the client is asked for, with the tunnel away.
const PROBE_BUDGET: Duration = Duration::from_secs(PROBE_SECONDS);

/// How long putting the tunnel back may take.
const RESTORE_BUDGET: Duration = Duration::from_secs(RESTORE_SECONDS);

/// The whole of what this takes away, and for how long.
///
/// The tunnel is down for the probe and then for the restore, and both are bounded, so
/// this is the longest a stack is ever without it. It is what the operator is told
/// before they opt in, and what the check keeps back out of its budget before it drops
/// anything — the two being the same value is what keeps the promise and the bound
/// from drifting apart.
pub(super) const DISTURBANCE: Duration = Duration::from_secs(PROBE_SECONDS + RESTORE_SECONDS);

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
            not_asked_for()
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
        Held::NotAttempted { reason } if *reason == not_asked_for() => Verdict::Unverified {
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
///
/// It says what running it costs and for how long, which is what an operator has to
/// weigh before opting in: the tunnel goes down, and transfers stop for as long as the
/// tunnel is allowed to be away rather than for as long as the check runs.
pub(super) fn not_asked_for() -> String {
    format!(
        "the killswitch has not been tested; proving it works means dropping the tunnel and \
         confirming traffic stops, which interrupts transfers {}",
        crate::doctor::disturbing_for(DISTURBANCE)
    )
}

/// Driving the test: taking the tunnel away and putting it back.
///
/// The verdicts above say what the result *means*; this is the doing of it, and
/// it is the only thing in the VPN check that changes the running system. Kept
/// with the verdicts rather than with the read-only observation for that reason —
/// what disturbs a stack and what merely looks at one are different kinds of
/// code, and one of them needs reading twice.
impl super::VpnCheck {
    /// Prove the killswitch: take the tunnel away, ask the client whether it can
    /// still reach the internet, and put the tunnel back.
    ///
    /// Only where the operator asked for the disruptive checks, and only where
    /// there is something to prove — a client that is not reaching the internet
    /// right now would answer "blocked" whatever the killswitch does, and calling
    /// that a pass would be the comfortable falsehood again in a new place.
    ///
    /// Whatever the probe finds, the tunnel goes back up. That restoration is
    /// attempted unconditionally and then confirmed; a tunnel this check took away
    /// and could not return is reported as the fault it is rather than left for
    /// the operator to discover.
    ///
    /// `deadline` is when the run this belongs to is abandoned. It is read before
    /// anything is dropped, because after that there is no reading it: the abandoning
    /// is a dropped future, which takes no path out at all.
    pub(super) async fn killswitch_held(
        &self,
        gateway: Option<&Container>,
        client: Option<&Container>,
        echo: &str,
        client_now: &Reach,
        deadline: Instant,
    ) -> Held {
        if !self.disruptive {
            return Held::NotAttempted {
                reason: not_asked_for(),
            };
        }
        let Some(gateway) = running(gateway) else {
            return not_attempted(true, "the tunnel container is not running");
        };
        let Some(client) = running(client) else {
            return not_attempted(true, "the download client is not running");
        };
        // Nothing to prove against: a client already unable to reach the internet
        // answers "blocked" whether or not the killswitch had anything to do with
        // it, and reading that as a pass would prove nothing at all.
        if !matches!(client_now, Reach::Address(_)) {
            return not_attempted(
                true,
                "the download client is not reaching the internet right now, so stopping it \
                 would prove nothing",
            );
        }
        let Some(device) = self.tunnel_device(gateway).await else {
            return not_attempted(
                true,
                "the tunnel container carries no default route to drop",
            );
        };
        // Everything above this only read. Below it the stack is disturbed, and the
        // whole disturbance has to fit in what is left of the run: a run that ends
        // mid-probe ends by dropping this future, which restores nothing.
        if deadline.saturating_duration_since(Instant::now()) < DISTURBANCE {
            return not_attempted(
                true,
                "too little of this check's time was left to drop the tunnel and be sure of \
                 putting it back",
            );
        }
        if !self.set_link(gateway, &device, false).await {
            return not_attempted(true, "the tunnel device could not be taken down");
        }

        // From here the stack is disturbed, so every path restores before it
        // returns — the probe's own answer is held until that has happened — and
        // no path here can outlast the budget that would drop it mid-way.
        //
        // The CLIENT is asked, not the gateway. The gateway's own traffic is not
        // the question; whether the container behind it still reaches the world
        // with the tunnel gone is the entire test. A client cut off with the tunnel
        // is also the likeliest thing in this check to stop answering altogether,
        // which is why the asking is bounded and the answer to a bound reached is
        // the same as the answer to a probe that failed: nothing was proven.
        let reached = timeout(PROBE_BUDGET, self.reach(Some(client), echo))
            .await
            .unwrap_or(Reach::Unknown);
        let held = held_from(&reached);
        if self.restored(gateway, &device).await {
            held
        } else {
            Held::NotRestored
        }
    }

    /// Put the tunnel back, and confirm it came back.
    ///
    /// Bounded like the probe, and for a sharper reason: an engine that never answers
    /// this would hold the run open for as long as it stayed silent. A tunnel that
    /// cannot be confirmed back inside that bound is reported as the emergency it is,
    /// which reaches the operator; waiting on it does not.
    async fn restored(&self, gateway: &Container, device: &str) -> bool {
        timeout(RESTORE_BUDGET, async {
            self.set_link(gateway, device, true).await
                && self.tunnel_device(gateway).await.is_some()
        })
        .await
        .unwrap_or(false)
    }

    /// Which device carries the gateway's default route — the tunnel, whatever it
    /// is called.
    async fn tunnel_device(&self, gateway: &Container) -> Option<String> {
        let output = self.engine.exec(&gateway.id, &read_route()).await.ok()?;
        tunnel_device(&output.stdout)
    }

    /// Take the gateway's tunnel device down, or put it back. Whether the engine
    /// carried the command out at all.
    async fn set_link(&self, gateway: &Container, device: &str, up: bool) -> bool {
        self.engine
            .exec(&gateway.id, &set_link(device, up))
            .await
            .is_ok_and(|output| output.status == Some(0))
    }
}

#[cfg(test)]
mod tests {
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
}
