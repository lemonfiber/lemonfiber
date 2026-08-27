//! One screen's worth of "what is my stack doing right now?", assembled rather
//! than fetched.
//!
//! The value of a dashboard is the fragments no single service can give: that the
//! download client's traffic is genuinely leaving through the tunnel, that imports
//! are hardlinking rather than copying, that the disk will fill before the queue
//! drains. This module is the shape of that screen and the rules that keep it
//! honest — nothing here reaches a daemon or an API. The surface gathers each
//! source through the ports and hands the pieces in; this decides what they add up
//! to and, above all, how they read when a source falls silent.
//!
//! Three distinctions are load-bearing. A figure that is current, one that is the
//! last thing a now-silent source said, and one that was never measured are three
//! different things ([`Reading`]) — "0 B/s" and "unknown" mean opposite things to
//! someone deciding whether a download is stuck. A panel that is live and one whose
//! source could not be reached are two different things ([`Panel`]), so one dead
//! source marks its own region and leaves the rest of the screen alone. And every
//! duration is computed from one clock and never runs backwards, since a host and a
//! container disagreeing about the time must not render as a negative countdown.

use std::time::Duration;

use serde::Serialize;

use crate::docker::Service;
use crate::health::{Reach, Summary};

/// A figure a source reports, kept apart from the two ways it can be missing.
///
/// Zero is a value a source gave; stale is the last value a source that has since
/// gone quiet gave; unknown is a source that never answered at all. Collapsing any
/// two of them sends an operator after the wrong problem — a stalled download and
/// a dashboard that simply stopped polling look identical only if the code lets
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "reading", content = "value")]
pub enum Reading<T> {
    /// The source answered this refresh with a value — which may legitimately be
    /// zero.
    Known(T),
    /// The source did not answer this refresh; this is the last value it gave.
    Stale(T),
    /// The source has never answered, so nothing can be said about it.
    Unknown,
}

impl<T> Reading<T> {
    /// Whether this reading is current — a `Known`, however small its value.
    #[must_use]
    pub const fn is_current(&self) -> bool {
        matches!(self, Self::Known(_))
    }

    /// The value, whether current or stale, or `None` where nothing was ever read.
    #[must_use]
    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Known(value) | Self::Stale(value) => Some(value),
            Self::Unknown => None,
        }
    }
}

impl<T: Clone> Reading<T> {
    /// This reading, or the last value a now-quiet source gave, marked stale.
    ///
    /// The middle state exists for exactly this: a source that answered a moment
    /// ago and did not this time has told us something, and blanking its figure to
    /// `unknown` throws that away — while presenting it as current would be a lie.
    /// Carried forward until a fresh value arrives, since it remains the last thing
    /// the source actually said.
    #[must_use]
    pub fn or_stale(self, previous: Option<&Self>) -> Self {
        match self {
            Self::Known(value) => Self::Known(value),
            Self::Stale(_) | Self::Unknown => match previous.and_then(Self::value) {
                Some(last) => Self::Stale(last.clone()),
                None => Self::Unknown,
            },
        }
    }
}

/// A panel's content, or the reason its source could not fill it.
///
/// The difference between "this panel is up to date" and "this panel's source is
/// unreachable" is the whole of degrading honestly: an unavailable panel says so,
/// in its own words, rather than showing stale data as current or blank data as
/// zero — and the panels beside it stay live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "panel", content = "data")]
pub enum Panel<T> {
    /// The source answered; here is the panel.
    Ready(T),
    /// The source could not be reached, for this stated reason.
    Unavailable {
        /// Why the panel could not be filled, in the operator's terms.
        reason: String,
    },
}

impl<T> Panel<T> {
    /// A panel whose source could not be reached.
    #[must_use]
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable {
            reason: reason.into(),
        }
    }

    /// Whether the panel could be filled.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Ready(_))
    }
}

/// Which protocol a transfer is moving over, since the same download reads
/// differently on each — a Usenet download has no peers, a torrent has no server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    /// A Usenet download.
    Usenet,
    /// A torrent download.
    Torrent,
}

/// One active download, as the dashboard shows it.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct Transfer {
    /// What is being downloaded.
    pub name: String,
    /// How it is being downloaded.
    pub protocol: Protocol,
    /// How far along, as a percentage from zero to a hundred.
    pub progress: u8,
    /// The current speed in bytes per second — a [`Reading`], because a genuine
    /// zero (stalled) and a source that has gone quiet mean opposite things here,
    /// and this is the very figure that difference is about.
    pub speed: Reading<u64>,
    /// The time left, or `None` where it is stalled and there is none to give.
    pub eta: Option<Duration>,
}

/// One `*arr`'s queue, and how much of it is stuck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Queue {
    /// The service whose queue this is.
    pub service: String,
    /// How many items are queued.
    pub depth: usize,
    /// How many of them are stuck rather than progressing.
    pub stuck: usize,
}

/// Whether imports are hardlinking or copying — the difference between an import
/// that is free and one that doubles the disk it uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Hardlink {
    /// Imports hardlink, as they should.
    Linking,
    /// Imports copy, doubling space and slowing every import.
    Copying,
    /// It could not be established which.
    Unknown,
}

/// The storage picture: what is free, when it runs out, and whether imports link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Storage {
    /// Bytes free on the data volume — a [`Reading`], since a volume that could
    /// not be read this refresh must not render as zero free.
    pub free: Reading<u64>,
    /// The time until the disk fills at the current rate of the queue draining
    /// onto it, or `None` where it is not projected to fill.
    pub exhaustion: Option<Duration>,
    /// Whether imports are linking or copying.
    pub hardlink: Hardlink,
}

/// What the VPN is doing, and whether the download client is actually behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Vpn {
    /// The tunnel's exit address as the outside world sees it.
    pub exit_ip: String,
    /// The country that address is in.
    pub country: String,
    /// The port the provider forwards, where forwarding is on.
    pub forwarded_port: Option<u16>,
    /// Whether the download client's own egress address matches the tunnel's —
    /// the one thing that proves traffic is genuinely leaving through it.
    pub egress_matches: bool,
}

/// How the screen itself is doing, which is a different question from how the
/// stack is doing.
///
/// The stack's own verdict is [`crate::health::Standing`]; this is only whether the
/// picture can be trusted to be current. Kept apart because they disagree in both
/// directions: a healthy stack can be shown through half-failing telemetry, and a
/// perfectly refreshing screen can be reporting a stack that is on fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Telemetry {
    /// Telemetry is current and refreshing normally.
    Live,
    /// Some sources are unavailable; their panels are marked.
    Degraded,
    /// The engine cannot be reached, though the CLI path may still work.
    Disconnected,
    /// Configured, but nothing is running.
    NoStack,
    /// No configuration; setup is offered instead.
    Unconfigured,
}

impl Telemetry {
    /// Read how the screen is doing from how far the surface reached and whether
    /// any panel's source was down.
    ///
    /// Ordered by how much is wrong: no configuration outranks everything, then an
    /// unreachable engine (telemetry is off but control may not be), then a stack
    /// that is configured but idle, and only on a reachable stack does a down panel
    /// mark the screen degraded rather than live. The order matters — a disconnected
    /// engine must not be hidden behind a "degraded" that suggests the screen is
    /// merely incomplete.
    ///
    /// A stack that is still coming up reads live: the screen is refreshing fine,
    /// and what the operator should make of a half-started stack is the health
    /// summary's job to say, not this one's.
    #[must_use]
    pub const fn read(reach: Reach, any_panel_down: bool) -> Self {
        match reach {
            Reach::Unconfigured => Self::Unconfigured,
            Reach::Unreachable => Self::Disconnected,
            Reach::Stopped => Self::NoStack,
            Reach::Running | Reach::Starting if any_panel_down => Self::Degraded,
            Reach::Running | Reach::Starting => Self::Live,
        }
    }
}

/// Everything the dashboard shows at one moment.
///
/// Each source's panel is filled or marked unavailable on its own, so one dead
/// source degrades one region rather than the screen. The surface builds this from
/// what it gathered; the standing is read from the same facts so it cannot
/// disagree with the panels.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct Snapshot {
    /// Whether the screen itself can be trusted to be current.
    pub telemetry: Telemetry,
    /// The one-line health summary — the same computation every other surface
    /// uses, so no two of them can grade the same stack differently.
    ///
    /// Always present, unlike the panels: a stack that could not be reached has a
    /// summary, and it says `unknown`. An absent summary would leave the operator
    /// to infer health from a blank space, which is the one reading this must never
    /// be open to.
    pub health: Summary,
    /// The VPN, or `None` where no VPN is configured and the panel is omitted
    /// rather than shown permanently red.
    pub vpn: Option<Panel<Vpn>>,
    /// The active transfers.
    pub transfers: Panel<Vec<Transfer>>,
    /// The per-service queues.
    pub queue: Panel<Vec<Queue>>,
    /// What in the pipeline has stopped, worst first — assessed across the
    /// download clients and the \*arrs together, because the failure that matters
    /// most is invisible inside either.
    pub stuck: Vec<crate::queue::Stuck>,
    /// What the operator has been told, newest first: what is owed them where a
    /// channel is refusing, then what has already been said.
    pub alerts: Vec<crate::alert::Alert>,
    /// The storage picture.
    pub storage: Panel<Storage>,
    /// Every service and what it is doing.
    pub services: Panel<Vec<Service>>,
    /// The one address to hand somebody who lives here.
    ///
    /// On the screen rather than only behind a question, because the operator who
    /// needs it is not the one who thought to ask: they have just been asked "what
    /// do I open?" by somebody in the next room. Built from the same reading as the
    /// panels beside it, so the screen and `front-door` cannot name different doors.
    pub door: Panel<crate::model::FrontDoorReport>,
}

/// The time to move `remaining` bytes at `speed` bytes per second.
///
/// A speed of zero yields no estimate rather than an infinite one: a stalled
/// download has no ETA, and rendering one as "∞" or as a wildly large number is
/// less honest than saying there is none.
#[must_use]
pub fn eta(remaining: u64, speed: u64) -> Option<Duration> {
    remaining.checked_div(speed).map(Duration::from_secs)
}

/// How far `done` is through `total`, as a percentage from zero to a hundred.
///
/// Kept in whole percent and integer arithmetic — a progress bar needs no more,
/// and it sidesteps the precision a float cast of a byte count would quietly lose.
/// A `total` of zero is complete rather than undefined — there is nothing left to
/// do — and a `done` past `total`, which clock or accounting skew can produce, is
/// clamped to a hundred rather than reported as more-than-finished.
#[must_use]
pub fn percent(done: u64, total: u64) -> u8 {
    let pct = u128::from(done)
        .saturating_mul(100)
        .checked_div(u128::from(total))
        .map_or(100, |ratio| ratio.min(100));
    u8::try_from(pct).unwrap_or(100)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        eta, percent, Hardlink, Panel, Protocol, Queue, Reach, Reading, Snapshot, Storage,
        Telemetry, Transfer, Vpn,
    };
    use crate::docker::{Service, State};
    use crate::health::Summary;
    use lemonfiber_manifest::Criticality;

    #[test]
    fn a_known_zero_is_current_and_distinct_from_unknown() {
        let zero: Reading<u64> = Reading::Known(0);
        let stale: Reading<u64> = Reading::Stale(42);
        let unknown: Reading<u64> = Reading::Unknown;

        assert!(zero.is_current(), "a known zero is a current value");
        assert!(!stale.is_current(), "stale is not current");
        assert!(!unknown.is_current());

        assert_eq!(zero.value(), Some(&0));
        assert_eq!(
            stale.value(),
            Some(&42),
            "stale still carries its last value"
        );
        assert_eq!(unknown.value(), None, "unknown carries nothing");
    }

    #[test]
    fn a_source_that_has_gone_quiet_keeps_its_last_value_rather_than_blanking() {
        // The middle state exists for exactly this. Blanking throws away something
        // the source told us; presenting it as current would be a lie.
        let last = Reading::Known(4096_u64);
        assert_eq!(
            Reading::Unknown.or_stale(Some(&last)),
            Reading::Stale(4096),
            "the last thing it actually said, marked as such"
        );
    }

    #[test]
    fn a_fresh_value_replaces_a_stale_one_rather_than_being_shadowed_by_it() {
        let last = Reading::Stale(4096_u64);
        assert_eq!(Reading::Known(0).or_stale(Some(&last)), Reading::Known(0));
    }

    #[test]
    fn a_source_that_has_never_answered_stays_unknown() {
        // Nothing to carry forward, and inventing a figure would be worse than the
        // blank this correctly reports.
        assert_eq!(Reading::<u64>::Unknown.or_stale(None), Reading::Unknown);
        let never: Reading<u64> = Reading::Unknown;
        assert_eq!(
            Reading::<u64>::Unknown.or_stale(Some(&never)),
            Reading::Unknown
        );
    }

    #[test]
    fn a_value_stays_stale_across_refreshes_until_something_fresh_arrives() {
        // It remains the last thing the source said, however long ago; a stale
        // reading that decayed to unknown would lose that on the second refresh.
        let once = Reading::Unknown.or_stale(Some(&Reading::Known(4096_u64)));
        assert_eq!(Reading::Unknown.or_stale(Some(&once)), Reading::Stale(4096));
    }

    #[test]
    fn a_panel_is_either_ready_or_states_why_not() {
        let ready = Panel::Ready(7);
        let down: Panel<i32> = Panel::unavailable("the service did not answer");
        assert!(ready.is_available());
        assert!(!down.is_available());
        assert!(matches!(down, Panel::Unavailable { reason } if reason.contains("did not answer")));
    }

    fn service(id: &str, state: State) -> Service {
        Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "media".to_owned(),
            state,
            criticality: Criticality::Core,
            exit: None,
            depends_on: Vec::new(),
        }
    }

    #[test]
    fn how_the_screen_is_doing_is_read_in_order_of_how_much_is_wrong() {
        // Unconfigured outranks all, then a disconnected engine, then a stopped
        // stack, then a degraded panel, then a fully live screen.
        assert_eq!(
            Telemetry::read(Reach::Unconfigured, false),
            Telemetry::Unconfigured
        );
        assert_eq!(
            Telemetry::read(Reach::Unreachable, true),
            Telemetry::Disconnected,
            "a disconnected engine is not hidden behind degraded"
        );
        assert_eq!(Telemetry::read(Reach::Stopped, false), Telemetry::NoStack);
        assert_eq!(Telemetry::read(Reach::Running, true), Telemetry::Degraded);
        assert_eq!(Telemetry::read(Reach::Running, false), Telemetry::Live);
    }

    #[test]
    fn a_stack_still_coming_up_leaves_the_screen_live() {
        // How the screen is doing and how the stack is doing are different
        // questions: telemetry refreshing normally over a half-started stack is a
        // live screen, and what to make of the stack is the summary's job.
        assert_eq!(Telemetry::read(Reach::Starting, false), Telemetry::Live);
        assert_eq!(Telemetry::read(Reach::Starting, true), Telemetry::Degraded);
    }

    #[test]
    fn an_eta_is_none_when_stalled_and_a_duration_otherwise() {
        assert_eq!(eta(1_000, 0), None, "a stalled transfer has no ETA");
        assert_eq!(eta(1_000, 100), Some(Duration::from_secs(10)));
        assert_eq!(
            eta(0, 100),
            Some(Duration::from_secs(0)),
            "done is zero, not none"
        );
    }

    #[test]
    fn a_host_and_a_container_disagreeing_about_the_time_cannot_render_a_countdown() {
        // Durations come from one clock, and the arithmetic is saturating either
        // way: a remaining count larger than the total, or a speed read after the
        // figure it divides, must not produce a negative or a wrapped duration.
        assert_eq!(eta(0, 1_000), Some(Duration::from_secs(0)), "already there");
        assert_eq!(eta(u64::MAX, 1), Some(Duration::from_secs(u64::MAX)));
        // A speed of zero is the disagreement made concrete: nothing is moving, so
        // there is no arrival time rather than an infinite or negative one.
        assert_eq!(eta(1_000, 0), None);
    }

    #[test]
    fn a_percentage_stays_between_zero_and_a_hundred() {
        assert_eq!(percent(0, 100), 0);
        assert_eq!(percent(50, 100), 50);
        assert_eq!(percent(100, 100), 100);
        assert_eq!(
            percent(150, 100),
            100,
            "past-total skew clamps to a hundred, never more than finished"
        );
        assert_eq!(
            percent(5, 0),
            100,
            "nothing to do is complete, not undefined"
        );
    }

    #[test]
    fn the_value_types_serialise_for_the_machine_readable_side() {
        let reading = Reading::Known(3_u64);
        let json = serde_json::to_string(&reading).unwrap_or_default();
        assert!(json.contains("known") && json.contains('3'), "{json}");

        let down: Panel<u8> = Panel::unavailable("offline");
        let json = serde_json::to_string(&down).unwrap_or_default();
        assert!(
            json.contains("unavailable") && json.contains("offline"),
            "{json}"
        );
    }

    #[test]
    fn a_whole_snapshot_serialises_with_each_panel_filled_or_marked() {
        // One dead source (the queue) is marked unavailable while the rest are live,
        // and the whole thing round-trips to the machine-readable form.
        let snapshot = Snapshot {
            telemetry: Telemetry::Degraded,
            health: Summary::of(Reach::Running, &[], "1000"),
            vpn: Some(Panel::Ready(Vpn {
                exit_ip: "203.0.113.7".to_owned(),
                country: "Netherlands".to_owned(),
                forwarded_port: Some(51413),
                egress_matches: true,
            })),
            transfers: Panel::Ready(vec![Transfer {
                name: "Some.Release".to_owned(),
                protocol: Protocol::Torrent,
                progress: percent(3, 4),
                speed: Reading::Known(1_048_576),
                eta: eta(5_000_000, 1_048_576),
            }]),
            queue: Panel::unavailable("sonarr did not answer"),
            stuck: Vec::new(),
            alerts: Vec::new(),
            storage: Panel::Ready(Storage {
                free: Reading::Known(42_000_000_000),
                exhaustion: None,
                hardlink: Hardlink::Linking,
            }),
            services: Panel::Ready(vec![service("sonarr", State::Healthy)]),
            door: Panel::Ready(crate::model::FrontDoorReport {
                standing: crate::model::Standing::Established,
                chosen: crate::door::Chosen::Derived,
                service: Some("Seerr".to_owned()),
                address: Some(crate::door::Address {
                    url: "http://kitchen-nas.local:5055".to_owned(),
                    caution: None,
                }),
                facing: Some(crate::door::Facing::Asking),
                meaning: "send them there".to_owned(),
                beside: Vec::new(),
            }),
        };

        let json = serde_json::to_string(&snapshot).unwrap_or_default();
        for expected in [
            "degraded",
            "203.0.113.7",
            "torrent",
            "linking",
            "sonarr did not answer",
            "http://kitchen-nas.local:5055",
        ] {
            assert!(json.contains(expected), "missing {expected} in {json}");
        }
        // The Usenet protocol, a filled queue, and the other hardlink states
        // serialise too, so every variant's rendering is exercised.
        let queue = Queue {
            service: "radarr".to_owned(),
            depth: 3,
            stuck: 1,
        };
        assert!(serde_json::to_string(&queue).is_ok_and(|json| json.contains("radarr")));
        for hardlink in [Hardlink::Copying, Hardlink::Unknown] {
            assert!(serde_json::to_string(&hardlink).is_ok());
        }
        assert!(serde_json::to_string(&Protocol::Usenet).is_ok_and(|json| json.contains("usenet")));
    }
}
