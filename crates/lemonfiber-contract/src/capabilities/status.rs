//! `status.watch`: what the monitor watching the stack from outside it is asked.
//!
//! It answers whether it answers and every monitor it holds, is told to make, update and
//! remove the monitors lemonfiber provisions, each under a tag of lemonfiber's own, and is
//! told of each check-in of a job lemonfiber runs on a beat.

use lemonfiber_ports::service::{
    Covers, Monitor, MonitorKind, Monitoring, Provision, Provisioned, Reach,
};

use super::NAMED;

/// The seconds between the check-ins the conformance cases' heartbeat expects, and the
/// seconds past them it waits.
const EVERY: u64 = 60;

/// How many check-ins in a row the conformance cases' heartbeat misses before it alarms.
const RETRIES: u32 = 1;

/// The tag of the heartbeat the conformance cases provision, check in to and retire.
fn tag() -> String {
    Covers::Job(NAMED.to_owned()).tag()
}

crate::contract! {
    /// `status.watch`: the outside watcher.
    pub mod watch = "status.watch" @ 1 holds [
        crate::Held::kept("monitors", "Monitors the operator made"),
        crate::Held::kept("readings", "Every reading it took"),
    ] {
        impl Monitoring {
            /// Whether it answers, and why not where it does not.
            fn reachable() -> Reach where asks = Asked {};
            /// Every monitor it holds.
            fn monitors() -> Vec<Monitor> where asks = Asked {};
            /// Make or update the one monitor holding a tag.
            fn provision(refer provision: &Provision as Provision) -> Provisioned where asks = Asked {
                provision: Provision {
                    tag: tag(),
                    kind: MonitorKind::Heartbeat,
                    target: NAMED.to_owned(),
                    every_s: EVERY,
                    grace_s: Some(EVERY),
                    retries: RETRIES,
                },
            };
            /// Record a check-in of the heartbeat holding a tag.
            fn check_in(str tag: &str) -> () where asks = Asked { tag: tag() };
            /// Remove the monitor holding a tag, and nothing where none does.
            fn retire(str tag: &str) -> () where asks = Asked { tag: tag() };
        }
    }
}
