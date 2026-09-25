use std::time::Duration;

use super::{
    eta, percent, Hardlink, Panel, Protocol, Queue, Reach, Reading, Snapshot, Storage, Telemetry,
    Transfer, Vpn,
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
        describes: format!("what {id} is for"),
        profile: "media".to_owned(),
        forms: Vec::new(),
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
        household: Panel::Ready(crate::model::HouseholdReport {
            members: Vec::new(),
            available: true,
            findings: Vec::new(),
            filtering: None,
            policy: None,
            allows: None,
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
