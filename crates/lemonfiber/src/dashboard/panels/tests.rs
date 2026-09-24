use super::{
    alerts, any_panel_down, front_door, header, household, queues, services, storage, stuck,
    transfers, vpn, SHOWN,
};
use lemonfiber_core::alert::Alert;
use lemonfiber_core::dashboard::{
    Hardlink, Panel, Protocol, Queue, Reading, Storage, Telemetry, Transfer, Vpn,
};
use lemonfiber_core::door::{Address, Chosen, Facing, Refusal};
use lemonfiber_core::health::{Reach, Summary};
use lemonfiber_core::household::State;
use lemonfiber_core::model::{
    FrontDoorReport, HouseholdMember, HouseholdReport, MemberRequest, Standing,
};
use lemonfiber_core::queue::Stuck;

/// A panel with more room than anything here is testing the edge of.
const WIDE: usize = 200;

/// The words of a panel, one string per line.
fn said(lines: &[ratatui::text::Line<'static>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect()
}

/// A household with one request in each state, for the panel to choose from.
fn asked(states: &[Option<State>]) -> Panel<HouseholdReport> {
    Panel::Ready(HouseholdReport {
        members: vec![HouseholdMember {
            name: "Ana".to_owned(),
            requests: states
                .iter()
                .enumerate()
                .map(|(at, state)| MemberRequest {
                    title: Some(format!("A film {at}")),
                    media: Some("film".to_owned()),
                    state: *state,
                    id: 0,
                    waiting_days: None,
                    estimate: None,
                    refused: None,
                })
                .collect(),
            ..HouseholdMember::default()
        }],
        available: true,
        findings: Vec::new(),
        filtering: None,
        policy: None,
        allows: None,
    })
}

/// Only the two states an operator can act on reach the panel.
///
/// A request being fetched or already here needs nobody. Listing it would push
/// the ones that do off a panel this size, which is the panel being worse than
/// empty.
#[test]
fn the_household_panel_shows_only_what_is_waiting_on_the_operator() {
    let lines = household(
        &asked(&[
            Some(State::WaitingForApproval),
            Some(State::Getting),
            Some(State::Here),
            Some(State::Failed),
        ]),
        WIDE,
    );
    let said = said(&lines);

    assert_eq!(said.len(), 2, "{said:?}");
    assert!(said.iter().any(|line| line.contains("waiting")), "{said:?}");
    assert!(said.iter().any(|line| line.contains("failed")), "{said:?}");
    assert!(
        !said.iter().any(|line| line.contains("A film 1")),
        "a request being fetched reached the panel: {said:?}"
    );
}

/// A household with nothing outstanding says so rather than drawing nothing.
#[test]
fn a_household_waiting_on_nothing_says_so() {
    let said = said(&household(&asked(&[Some(State::Here)]), WIDE));

    assert_eq!(said, vec!["nothing is waiting on you".to_owned()]);
}

/// A panel that could not be filled says why, as every other panel does.
///
/// Distinct from the case below: this is the reading failing before Seerr is
/// reached at all — an unreadable stack — where that one is Seerr reached and
/// not answering. Both leave an empty list and they are not the same thing.
#[test]
fn a_household_panel_that_could_not_be_filled_says_why() {
    let said = said(&household(
        &Panel::unavailable("the stack could not be read"),
        WIDE,
    ));

    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said.first()
            .is_some_and(|line| line.contains("the stack could not be read")),
        "{said:?}"
    );
}

/// A request service that could not be read is said to be unread.
///
/// Distinct from a household that has asked for nothing: the same distinction
/// the report itself keeps, carried onto the screen rather than flattened there.
#[test]
fn a_request_service_that_was_not_read_says_so_rather_than_looking_empty() {
    let unread = Panel::Ready(HouseholdReport {
        members: Vec::new(),
        available: false,
        findings: vec!["seerr did not answer".to_owned()],
        filtering: None,
        policy: None,
        allows: None,
    });
    let said = said(&household(&unread, WIDE));

    assert_eq!(said, vec!["the household was not read".to_owned()]);
}

/// One transfer, however it is going.
fn transfer(name: &str, speed: Reading<u64>) -> Transfer {
    Transfer {
        name: name.to_owned(),
        protocol: Protocol::Usenet,
        progress: 42,
        speed,
        eta: Some(std::time::Duration::from_secs(600)),
    }
}

#[test]
fn a_stalled_download_and_a_client_that_stopped_talking_do_not_read_alike() {
    // The distinction the whole panel exists for: nought bytes a second is a
    // stalled download; no answer is a client that went quiet, and an operator
    // who cannot tell them apart learns to trust neither.
    let stalled = said(&transfers(
        &Panel::Ready(vec![transfer("Some.Release", Reading::Known(0))]),
        WIDE,
    ));
    let quiet = said(&transfers(
        &Panel::Ready(vec![transfer("Some.Release", Reading::Unknown)]),
        WIDE,
    ));
    assert_ne!(stalled, quiet);
    assert!(stalled.first().is_some_and(|line| line.contains("0 MB/s")));
    assert!(quiet.first().is_some_and(|line| line.contains("not read")));
}

#[test]
fn a_stale_speed_is_shown_and_marked_rather_than_dropped() {
    // The last known speed says more than a blank, as long as nobody reads it
    // as current.
    let stale = said(&transfers(
        &Panel::Ready(vec![transfer("Some.Release", Reading::Stale(5_000_000))]),
        WIDE,
    ));
    assert!(stale.first().is_some_and(|line| line.contains("5 MB/s ·")));
}

#[test]
fn an_empty_panel_says_so_rather_than_rendering_a_blank_region() {
    // A blank region reads as "nothing wrong", which is the one thing an empty
    // panel must never say on its own.
    assert_eq!(
        said(&transfers(&Panel::Ready(Vec::new()), WIDE)),
        vec!["nothing is downloading".to_owned()]
    );
    assert_eq!(
        said(&queues(&Panel::Ready(Vec::new()), WIDE)),
        vec!["no service reported a queue".to_owned()]
    );
    assert_eq!(
        said(&services(&Panel::Ready(Vec::new()), WIDE)),
        vec!["no services are running".to_owned()]
    );
}

#[test]
fn an_unavailable_panel_says_why_and_leaves_the_rest_alone() {
    let down: Panel<Vec<Transfer>> = Panel::unavailable("the client did not answer");
    assert_eq!(
        said(&transfers(&down, WIDE)),
        vec!["unavailable — the client did not answer".to_owned()]
    );
}

#[test]
fn a_long_list_is_cut_and_says_it_was() {
    // A truncated list that does not say it was truncated is one an operator
    // reads as complete.
    let many: Vec<Transfer> = (0..SHOWN + 3)
        .map(|n| transfer(&format!("Release.{n}"), Reading::Known(1_000_000)))
        .collect();
    let lines = said(&transfers(&Panel::Ready(many), WIDE));
    assert_eq!(lines.len(), SHOWN + 1);
    assert!(lines
        .last()
        .is_some_and(|line| line == "and 3 more transfers — 9 in all"));
}

#[test]
fn a_list_that_fits_says_nothing_about_a_rest() {
    let few: Vec<Transfer> = (0..2)
        .map(|n| transfer(&format!("Release.{n}"), Reading::Known(1_000_000)))
        .collect();
    assert_eq!(said(&transfers(&Panel::Ready(few), WIDE)).len(), 2);
}

#[test]
fn no_vpn_configured_is_stated_rather_than_shown_as_a_red_panel() {
    assert_eq!(
        said(&vpn(None, WIDE)),
        vec!["no VPN is configured".to_owned()]
    );
}

#[test]
fn the_vpn_panel_leads_with_whether_traffic_is_actually_inside_the_tunnel() {
    // A tunnel being up says nothing about whether the client's traffic is in
    // it, which is the only thing that proves anything.
    let leaking = Panel::Ready(Vpn {
        exit_ip: "203.0.113.7".to_owned(),
        country: "nl".to_owned(),
        forwarded_port: None,
        egress_matches: false,
    });
    let lines = said(&vpn(Some(&leaking), WIDE));
    assert!(lines
        .iter()
        .any(|line| line.contains("NOT inside the tunnel")));
    assert!(lines.iter().any(|line| line.contains("no forwarded port")));

    let behind = Panel::Ready(Vpn {
        exit_ip: "203.0.113.7".to_owned(),
        country: "nl".to_owned(),
        forwarded_port: Some(51413),
        egress_matches: true,
    });
    let lines = said(&vpn(Some(&behind), WIDE));
    assert!(lines.iter().any(|line| line.contains("leaves through")));
    assert!(lines.iter().any(|line| line.contains("port 51413")));
}

#[test]
fn an_unavailable_vpn_panel_says_why() {
    let down: Panel<Vpn> = Panel::unavailable("the gateway did not answer");
    assert_eq!(
        said(&vpn(Some(&down), WIDE)),
        vec!["unavailable — the gateway did not answer".to_owned()]
    );
}

#[test]
fn a_volume_that_could_not_be_read_never_reads_as_a_full_one() {
    // Opposite things to an operator: one is a fault in the reading, the other
    // is a fault they have to act on tonight.
    let unread = said(&storage(
        &Panel::Ready(Storage {
            free: Reading::Unknown,
            exhaustion: None,
            hardlink: Hardlink::Unknown,
        }),
        WIDE,
    ));
    assert!(unread
        .first()
        .is_some_and(|line| line.contains("could not be read")));

    let empty = said(&storage(
        &Panel::Ready(Storage {
            free: Reading::Known(0),
            exhaustion: None,
            hardlink: Hardlink::Linking,
        }),
        WIDE,
    ));
    assert!(empty.first().is_some_and(|line| line.contains("0 MB free")));
}

#[test]
fn copying_imports_are_stated_as_what_they_cost() {
    // The consequence rather than the property: "no hardlinks" is a filesystem
    // fact, and "twice the disk" is what the operator has to weigh.
    let copying = said(&storage(
        &Panel::Ready(Storage {
            free: Reading::Known(1_000_000_000),
            exhaustion: Some(std::time::Duration::from_secs(7200)),
            hardlink: Hardlink::Copying,
        }),
        WIDE,
    ));
    assert!(copying.iter().any(|line| line.contains("twice the disk")));
    assert!(copying.iter().any(|line| line.contains("full in ~2h")));
}

#[test]
fn a_queue_with_nothing_stuck_says_so_quietly() {
    let lines = said(&queues(
        &Panel::Ready(vec![Queue {
            service: "sonarr".to_owned(),
            depth: 4,
            stuck: 0,
        }]),
        WIDE,
    ));
    assert!(lines
        .first()
        .is_some_and(|line| line.contains("4 queued") && line.contains("none stuck")));
}

#[test]
fn a_queue_with_something_stuck_says_how_many() {
    let lines = said(&queues(
        &Panel::Ready(vec![Queue {
            service: "sonarr".to_owned(),
            depth: 4,
            stuck: 2,
        }]),
        WIDE,
    ));
    assert!(lines.first().is_some_and(|line| line.contains("2 stuck")));
}

#[test]
fn an_unavailable_queue_or_storage_or_service_panel_says_why() {
    let queue: Panel<Vec<Queue>> = Panel::unavailable("the stack could not be read");
    let store: Panel<Storage> = Panel::unavailable("no data location is configured");
    let running: Panel<Vec<lemonfiber_core::docker::Service>> =
        Panel::unavailable("the engine did not answer");
    assert!(said(&queues(&queue, WIDE))
        .first()
        .is_some_and(|line| line.starts_with("unavailable —")));
    assert!(said(&storage(&store, WIDE))
        .first()
        .is_some_and(|line| line.starts_with("unavailable —")));
    assert!(said(&services(&running, WIDE))
        .first()
        .is_some_and(|line| line.starts_with("unavailable —")));
}

#[test]
fn the_header_says_how_the_screen_is_doing_and_how_the_stack_is() {
    // Two different questions: a healthy stack can be shown through half-failing
    // telemetry, and a perfect screen can be reporting a stack that is on fire.
    let summary = Summary::of(Reach::Running, &[], "1000");
    let line = header(Telemetry::Degraded, &summary, WIDE);
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.to_string())
        .collect();
    assert!(text.contains("some sources are down"), "{text}");
    assert!(text.contains(&summary.said()), "{text}");
}

#[test]
fn every_way_the_screen_can_be_doing_has_words_of_its_own() {
    let summary = Summary::of(Reach::Running, &[], "1000");
    let mut said: Vec<String> = [
        Telemetry::Live,
        Telemetry::Degraded,
        Telemetry::Disconnected,
        Telemetry::NoStack,
        Telemetry::Unconfigured,
    ]
    .into_iter()
    .map(|telemetry| {
        header(telemetry, &summary, WIDE)
            .spans
            .iter()
            .map(|span| span.content.to_string())
            .collect::<String>()
    })
    .collect();
    said.sort_unstable();
    let total = said.len();
    said.dedup();
    assert_eq!(said.len(), total, "two states reading alike is one state");
}

#[test]
fn a_panel_that_could_not_be_filled_marks_the_screen_degraded() {
    // The panels decide it rather than a second reading of the same facts, so
    // the header and the panels cannot disagree.
    let mut snapshot = crate::dashboard::tests::a_snapshot();
    assert!(!any_panel_down(&snapshot));
    snapshot.storage = Panel::unavailable("no data location is configured");
    assert!(any_panel_down(&snapshot));

    // Each panel on its own, so one left out of the check is caught here rather
    // than by a screen that reads live while a panel of it is not.
    let mut one = crate::dashboard::tests::a_snapshot();
    one.household = Panel::unavailable("the request service did not answer");
    assert!(any_panel_down(&one));
}

/// The answer a stack with a working door gives.
fn door(chosen: Chosen, address: Option<Address>) -> FrontDoorReport {
    FrontDoorReport {
        standing: Standing::Established,
        chosen,
        service: Some("Seerr".to_owned()),
        address,
        facing: Some(Facing::Asking),
        meaning: "a sentence far too long for a panel".to_owned(),
        beside: Vec::new(),
    }
}

/// The address a machine that says what it is called is reached at.
fn reachable() -> Address {
    Address {
        url: "http://kitchen-nas.local:5055".to_owned(),
        caution: None,
    }
}

#[test]
fn the_screen_carries_the_address_to_hand_the_household() {
    // The whole point of the panel: the operator asked "what do I open?" by
    // somebody in the next room reads it off the screen they already have open.
    let lines = said(&front_door(
        &Panel::Ready(door(Chosen::Derived, Some(reachable()))),
        WIDE,
    ));
    assert_eq!(
        lines,
        vec![
            "Seerr  the front door".to_owned(),
            "http://kitchen-nas.local:5055".to_owned(),
        ]
    );
}

#[test]
fn a_door_with_no_address_shows_the_standing_and_no_line_to_read_out() {
    let lines = said(&front_door(
        &Panel::Ready(FrontDoorReport {
            standing: Standing::Stranded,
            address: None,
            ..door(Chosen::Derived, None)
        }),
        WIDE,
    ));
    assert_eq!(
        lines,
        vec!["Seerr  the front door, and there is no address to arrive at".to_owned()]
    );
}

#[test]
fn a_stack_with_no_door_says_so_rather_than_showing_an_empty_panel() {
    let lines = said(&front_door(
        &Panel::Ready(FrontDoorReport {
            standing: Standing::Absent,
            service: None,
            facing: None,
            ..door(Chosen::Derived, None)
        }),
        WIDE,
    ));
    assert_eq!(lines, vec!["There is no front door.".to_owned()]);
}

#[test]
fn a_setting_that_was_refused_is_on_the_screen_rather_than_only_in_the_answer() {
    // An operator watching this screen never asks the long question, so a
    // refusal that only appeared there is one they would never learn about.
    let lines = said(&front_door(
        &Panel::Ready(door(
            Chosen::Refused(Refusal {
                named: "sonarr".to_owned(),
                because: "it answers this machine alone".to_owned(),
            }),
            Some(reachable()),
        )),
        WIDE,
    ));
    assert_eq!(
        lines.last().map(String::as_str),
        Some("`sonarr` was named as the front door and cannot be one")
    );
}

#[test]
fn a_door_the_operator_named_says_that_it_was_named() {
    let lines = said(&front_door(
        &Panel::Ready(door(
            Chosen::Named("jellyfin".to_owned()),
            Some(reachable()),
        )),
        WIDE,
    ));
    assert_eq!(
        lines.last().map(String::as_str),
        Some("named rather than worked out")
    );
}

#[test]
fn a_door_panel_that_could_not_be_filled_says_why_and_marks_the_screen() {
    let mut snapshot = crate::dashboard::tests::a_snapshot();
    snapshot.door = Panel::unavailable("the stack could not be read");
    assert!(any_panel_down(&snapshot));
    let lines = said(&front_door(&snapshot.door, WIDE));
    assert_eq!(
        lines,
        vec!["unavailable — the stack could not be read".to_owned()]
    );
}

#[test]
fn a_vpn_panel_that_could_not_be_filled_marks_it_too() {
    let mut snapshot = crate::dashboard::tests::a_snapshot();
    snapshot.vpn = Some(Panel::unavailable("the gateway did not answer"));
    assert!(any_panel_down(&snapshot));
}

#[test]
fn a_transfer_with_no_estimate_says_so_rather_than_inventing_one() {
    // A stalled download has no arrival time, and rendering one as a wild
    // number is less honest than saying there is none.
    let stalled = Transfer {
        eta: None,
        ..transfer("Some.Release", Reading::Known(0))
    };
    let lines = said(&transfers(&Panel::Ready(vec![stalled]), WIDE));
    assert!(lines
        .first()
        .is_some_and(|line| line.contains("no estimate")));
}

#[test]
fn a_free_space_reading_that_went_stale_is_shown_and_marked() {
    let stale = said(&storage(
        &Panel::Ready(Storage {
            free: Reading::Stale(2_000_000_000),
            exhaustion: None,
            hardlink: Hardlink::Unknown,
        }),
        WIDE,
    ));
    assert!(stale
        .first()
        .is_some_and(|line| line.contains("2.0 GB free ·")));
}

#[test]
fn a_service_is_named_with_what_it_is_doing() {
    let running = lemonfiber_core::docker::Service {
        id: "sonarr".to_owned(),
        name: "Sonarr".to_owned(),
        describes: "Watches for new episodes and fetches them".to_owned(),
        profile: "tv".to_owned(),
        forms: Vec::new(),
        state: lemonfiber_core::docker::State::Running,
        criticality: lemonfiber_core::docker::Criticality::Core,
        depends_on: Vec::new(),
        exit: None,
    };
    let lines = said(&services(&Panel::Ready(vec![running]), WIDE));
    assert!(lines
        .first()
        .is_some_and(|line| line.contains("sonarr") && line.contains("running")));
}

#[test]
fn a_release_name_from_an_indexer_cannot_take_over_the_screen() {
    // Every name on this screen came from somewhere else. A terminal reads a
    // control character in the middle of one as an instruction, and the result
    // is a screen that no longer says what this product said.
    let named = transfer("Some\u{1b}[2JRelease", Reading::Known(1_000_000));
    let lines = said(&transfers(&Panel::Ready(vec![named]), WIDE));
    assert!(
        lines.first().is_some_and(|line| !line.contains('\u{1b}')),
        "{lines:?}"
    );
    assert!(
        lines
            .first()
            .is_some_and(|line| line.contains("Some[2JRelease")),
        "and the name it leaves is still the name: {lines:?}"
    );
}

/// One alert about something that happened.
fn an_alert(summary: &str, moment: lemonfiber_core::alert::Moment) -> Alert {
    Alert {
        check: "service.sonarr".to_owned(),
        kind: "service.down".to_owned(),
        moment,
        severity: lemonfiber_core::error::Severity::Warning,
        summary: summary.to_owned(),
        meaning: "nothing that needs it is working".to_owned(),
        remedies: vec!["start it".to_owned()],
        affected: vec!["service.sonarr".to_owned()],
    }
}

#[test]
fn an_alert_says_which_way_it_went_as_well_as_what_happened() {
    // "Sonarr is down" and "Sonarr came back" are opposite news, and a list
    // that renders them alike is one an operator has to read twice.
    let lines = said(&alerts(
        &[
            an_alert(
                "Sonarr is not running",
                lemonfiber_core::alert::Moment::Onset,
            ),
            an_alert(
                "Sonarr is running again",
                lemonfiber_core::alert::Moment::Resolved,
            ),
        ],
        WIDE,
    ));
    assert!(lines
        .first()
        .is_some_and(|line| line.starts_with("started")));
    assert!(lines
        .get(1)
        .is_some_and(|line| line.starts_with("resolved")));
}

/// The alerts panel is cut like every other list panel, and says so — a panel
/// that showed six of eight in silence would be read as the whole of what
/// happened.
#[test]
fn a_long_run_of_alerts_is_cut_and_says_it_was() {
    let many: Vec<Alert> = (0..SHOWN + 2)
        .map(|n| {
            an_alert(
                &format!("Sonarr said {n}"),
                lemonfiber_core::alert::Moment::Onset,
            )
        })
        .collect();

    let lines = said(&alerts(&many, WIDE));

    assert_eq!(lines.len(), SHOWN + 1);
    assert!(
        lines
            .last()
            .is_some_and(|line| line == "and 2 more alerts — 8 in all"),
        "{lines:?}"
    );
}

#[test]
fn a_quiet_stack_says_nothing_has_needed_saying() {
    // Not a blank region: an empty alerts panel is good news, and it should
    // read as good news rather than as a panel that failed to fill.
    assert_eq!(
        said(&alerts(&[], WIDE)),
        vec!["nothing has needed saying".to_owned()]
    );
}

#[test]
fn an_alert_from_a_service_cannot_take_over_the_screen() {
    // The summary quotes what a service said, which came from somewhere else.
    let evil = an_alert("gone\u{1b}[2Jaway", lemonfiber_core::alert::Moment::Onset);
    assert!(said(&alerts(&[evil], WIDE))
        .first()
        .is_some_and(|line| !line.contains('\u{1b}')));
}

#[test]
fn a_stuck_item_says_what_is_wrong_and_for_how_long() {
    let held = Stuck {
        name: "Some.Release".to_owned(),
        stall: lemonfiber_core::queue::Stall::StalledDownload,
        held_for: 7 * 60 * 60,
        blocking: None,
        items: 1,
    };
    let lines = said(&stuck(std::slice::from_ref(&held), WIDE));
    assert!(lines
        .first()
        .is_some_and(|line| line.contains("Some.Release") && line.contains("7 hours")));
}

/// The narrowest a panel gets before the screen stops offering two of them.
///
/// At the width where two columns begin, a row is halved and each place gives
/// up its borders, so this is the smallest room a panel is asked to work in
/// while the screen still claims to present two.
const NARROW: usize = (crate::dashboard::TWO_COLUMNS as usize / 2) - 2;

/// Nothing a panel writes for itself runs past the room it was given.
///
/// A value is shortened to fit and a sentence the product wrote is not, so a
/// sentence longer than the narrowest place is one the screen cuts — and the
/// existing tests ask these panels for two hundred columns, which is a width
/// this dashboard never gives them.
#[test]
fn no_panel_writes_a_line_longer_than_its_narrowest_place() {
    /// The lines of a panel that run past the room it was given.
    fn over(what: &str, lines: &[ratatui::text::Line<'static>], room: usize) -> Vec<String> {
        said(lines)
            .into_iter()
            .filter(|line| line.chars().count() > room)
            .map(|line| format!("{what}: {} chars — {line}", line.chars().count()))
            .collect()
    }

    let vpn_out = Panel::Ready(Vpn {
        exit_ip: "203.0.113.7".to_owned(),
        country: "nl".to_owned(),
        forwarded_port: None,
        egress_matches: false,
    });
    let vpn_in = Panel::Ready(Vpn {
        exit_ip: "203.0.113.7".to_owned(),
        country: "nl".to_owned(),
        forwarded_port: Some(51413),
        egress_matches: true,
    });

    let mut found: Vec<String> = Vec::new();
    found.extend(over(
        "vpn, traffic outside",
        &vpn(Some(&vpn_out), NARROW),
        NARROW,
    ));
    found.extend(over(
        "vpn, traffic inside",
        &vpn(Some(&vpn_in), NARROW),
        NARROW,
    ));
    found.extend(over("vpn, absent", &vpn(None, NARROW), NARROW));
    found.extend(over("alerts, none", &alerts(&[], NARROW), NARROW));
    found.extend(over("stuck, none", &stuck(&[], NARROW), NARROW));

    assert!(
        found.is_empty(),
        "these run past the {NARROW} columns the narrowest place gives them: {found:#?}"
    );

    // The measure itself, so what the assertion above rests on is exercised: a
    // line one column too long is reported, and the same line at its room is not.
    let long = ratatui::text::Line::from("x".repeat(NARROW + 1));
    assert_eq!(over("probe", std::slice::from_ref(&long), NARROW).len(), 1);
    assert!(over("probe", std::slice::from_ref(&long), NARROW + 1).is_empty());
}

#[test]
fn a_pipeline_with_nothing_stuck_says_so() {
    assert_eq!(said(&stuck(&[], WIDE)), vec!["nothing is stuck".to_owned()]);
}
