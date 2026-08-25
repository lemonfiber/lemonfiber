//! What each panel says, as lines.
//!
//! Lines rather than widgets, because what a panel *says* is the part worth
//! proving and a widget is only where it is put. Every one of these is a pure
//! function over the snapshot, so the screen's words are tested without a
//! terminal anywhere near them.
//!
//! Three distinctions run through all of it, and they are the reason this is not
//! a formatting exercise:
//!
//! * A **zero** and a source that went **quiet** are opposite things. Nought bytes
//!   a second is a stalled download; no answer is a client that stopped talking,
//!   and rendering them alike is how an operator comes to trust a number that is
//!   not being read.
//! * A **stale** value is worth showing, and worth marking. The last known speed
//!   tells more than a blank, as long as nobody reads it as current.
//! * An **absent** panel says why. An empty region reads as "nothing wrong".
//!
//! A panel is a box of a fixed height, so a row that will not fit is shortened
//! rather than wrapped: wrapping would push the last entries out of a box that
//! cannot grow, which trades a loss that is marked for one that is silent. What is
//! shortened is only what came from somewhere else — every value here reaches the
//! screen through [`plain`], and it is shortened in the middle, so the ends that
//! tell two of them apart both survive.

use crate::render::downloads::protocol;
use lemonfiber_core::alert::Alert;
use lemonfiber_core::dashboard::{
    Hardlink, Panel, Queue, Reading, Snapshot, Storage, Telemetry, Transfer, Vpn,
};
use lemonfiber_core::docker::Service;
use lemonfiber_core::health::Summary;
use lemonfiber_core::queue::Stuck;
use lemonfiber_core::text::{fitted, plain};
use lemonfiber_core::walkthrough::{size, spell_out};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// How many rows of a list are shown before the rest becomes a count.
///
/// A long list is not more information — past a handful it is a wall an operator
/// stops reading, and the thing that needed attention is somewhere in it.
const SHOWN: usize = 6;

/// How wide the service column is in the queues panel, where there is room for it.
///
/// A column narrower than the room left is what makes a list scannable down its
/// edge; where the room is narrower than the column, the room decides.
const NAMED: usize = 12;

/// The same, in the services panel, whose names sit beside a longer word.
const LISTED: usize = 14;

/// The dimmed style everything uncertain is drawn in.
fn quiet() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

/// The one-line header: whether the screen can be trusted, and what the stack
/// amounts to.
///
/// Two different questions, deliberately side by side. A healthy stack can be
/// shown through half-failing telemetry, and a perfectly refreshing screen can be
/// reporting a stack that is on fire.
pub(super) fn header(telemetry: Telemetry, health: &Summary, across: usize) -> Line<'static> {
    let state = screen(telemetry);
    let led = "lemonfiber  ".len() + state.chars().count() + "   ".len();
    Line::from(vec![
        Span::raw("lemonfiber  "),
        Span::styled(state.to_owned(), quiet()),
        Span::raw("   "),
        Span::raw(shortened(&health.said(), across.saturating_sub(led))),
    ])
}

/// Text from somewhere else, made safe for a terminal and then made to fit the row
/// it has.
///
/// One place, so no panel can put a value on the screen by a route that skips
/// either half of it.
fn shortened(value: &str, room: usize) -> String {
    fitted(&plain(value), room)
}

/// How the screen itself is doing, in a word.
const fn screen(telemetry: Telemetry) -> &'static str {
    match telemetry {
        Telemetry::Live => "live",
        Telemetry::Degraded => "some sources are down",
        Telemetry::Disconnected => "the container engine cannot be reached",
        Telemetry::NoStack => "nothing is running",
        Telemetry::Unconfigured => "not set up",
    }
}

/// The VPN panel: where traffic leaves from, and whether it is genuinely leaving
/// through the tunnel.
pub(super) fn vpn(panel: Option<&Panel<Vpn>>, room: usize) -> Vec<Line<'static>> {
    let Some(panel) = panel else {
        // No VPN configured. Said rather than shown as a permanently red panel,
        // which is what an omitted one would read as after a while.
        return vec![Line::styled("no VPN is configured", quiet())];
    };
    let vpn = match panel {
        Panel::Ready(vpn) => vpn,
        Panel::Unavailable { reason } => return unavailable(reason, room),
    };
    let port = vpn.forwarded_port.map_or_else(
        || Span::styled("no forwarded port", quiet()),
        |port| Span::raw(format!("port {port}")),
    );
    // The address takes the room it needs and the country takes what is left: two
    // values on one row, and the first of them is the one being checked.
    let exit = shortened(&vpn.exit_ip, room);
    let country = shortened(&vpn.country, room.saturating_sub(exit.chars().count() + 2));
    vec![
        Line::from(vec![Span::raw(exit), Span::raw("  "), Span::raw(country)]),
        Line::from(vec![
            // The one line that matters: a tunnel being up says nothing about
            // whether the client's traffic is inside it.
            if vpn.egress_matches {
                Span::raw("the client's traffic leaves through the tunnel")
            } else {
                Span::raw("the client's traffic is NOT going through the tunnel")
            },
        ]),
        Line::from(vec![port]),
    ]
}

/// The transfers panel: what is arriving, how fast, and when it lands.
pub(super) fn transfers(panel: &Panel<Vec<Transfer>>, room: usize) -> Vec<Line<'static>> {
    let transfers = match panel {
        Panel::Ready(transfers) => transfers,
        Panel::Unavailable { reason } => return unavailable(reason, room),
    };
    if transfers.is_empty() {
        return vec![Line::styled("nothing is downloading", quiet())];
    }
    let mut lines: Vec<Line<'static>> = transfers
        .iter()
        .take(SHOWN)
        .map(|transfer| {
            let led = vec![
                Span::raw(format!("{:>3}%  ", transfer.progress)),
                Span::raw(format!("{:<7}", protocol(transfer.protocol))),
                speed(&transfer.speed),
                Span::raw("  "),
                transfer.eta.map_or_else(
                    || Span::styled("no estimate", quiet()),
                    |left| Span::raw(format!("~{}", spell_out(left))),
                ),
                Span::raw("  "),
            ];
            let mut line = Line::from(led);
            // From an indexer by way of a download client: a terminal reads a
            // control character in the middle of it as an instruction, and the
            // name is what tells this download from the next one.
            line.push_span(Span::raw(shortened(
                &transfer.name,
                room.saturating_sub(width(&line)),
            )));
            line
        })
        .collect();
    lines.extend(rest(transfers.len(), "transfer"));
    lines
}

/// A speed, with a zero and a silence told apart.
fn speed(reading: &Reading<u64>) -> Span<'static> {
    match reading {
        Reading::Known(bytes) => Span::raw(format!("{:>9}/s", size(*bytes))),
        // Worth showing and worth marking: the last speed says more than a blank,
        // as long as nobody reads it as current.
        Reading::Stale(bytes) => Span::styled(format!("{:>9}/s ·", size(*bytes)), quiet()),
        Reading::Unknown => Span::styled(format!("{:>11}", "not read"), quiet()),
    }
}

/// The alerts panel: what the operator has been told, newest first.
///
/// The screen is a channel — the one that needs no configuring and cannot be down
/// — so an alert reaches here whether or not anything else was set up. What is
/// owed and what has been said read alike, because to somebody looking at the
/// screen there is no difference: both are things that happened.
pub(super) fn alerts(alerts: &[Alert], room: usize) -> Vec<Line<'static>> {
    if alerts.is_empty() {
        return vec![Line::styled("nothing has needed saying", quiet())];
    }
    alerts
        .iter()
        .map(|alert| {
            let moment = format!("{:<9}", alert.moment.said());
            let left = room.saturating_sub(moment.chars().count());
            Line::from(vec![
                Span::raw(moment),
                Span::raw(shortened(&alert.summary, left)),
            ])
        })
        .collect()
}

/// The stuck panel: what in the pipeline has stopped, and for how long.
///
/// Read across the download clients and the \*arrs together, because the failure
/// that matters most is invisible inside either: an item that downloaded and was
/// never imported is a finished download to the client and nothing at all to the
/// \*arr.
pub(super) fn stuck(stuck: &[Stuck], room: usize) -> Vec<Line<'static>> {
    if stuck.is_empty() {
        return vec![Line::styled("nothing is stuck", quiet())];
    }
    let mut lines: Vec<Line<'static>> = stuck
        .iter()
        .take(SHOWN)
        .map(|stuck| Line::from(vec![Span::raw(shortened(&stuck.said(), room))]))
        .collect();
    lines.extend(rest(stuck.len(), "item"));
    lines
}

/// The queue panel: how deep each service's queue is, and how much of it is stuck.
pub(super) fn queues(panel: &Panel<Vec<Queue>>, room: usize) -> Vec<Line<'static>> {
    let queues = match panel {
        Panel::Ready(queues) => queues,
        Panel::Unavailable { reason } => return unavailable(reason, room),
    };
    if queues.is_empty() {
        return vec![Line::styled("no service reported a queue", quiet())];
    }
    let mut lines: Vec<Line<'static>> = queues
        .iter()
        .take(SHOWN)
        .map(|queue| {
            let stuck = if queue.stuck == 0 {
                Span::styled("none stuck".to_owned(), quiet())
            } else {
                Span::raw(format!("{} stuck", queue.stuck))
            };
            let depth = format!("{:>4} queued  ", queue.depth);
            let led = depth.chars().count() + stuck.content.chars().count();
            // The column is what a short name is padded to, not what a long one is
            // held to: a name takes the whole of the room left before it is
            // shortened, and the column only lines the short ones up.
            let allowed = room.saturating_sub(led);
            let column = NAMED.min(allowed);
            let named = shortened(&queue.service, allowed);
            Line::from(vec![
                Span::raw(format!("{named:<column$}")),
                Span::raw(depth),
                stuck,
            ])
        })
        .collect();
    lines.extend(rest(queues.len(), "service"));
    lines
}

/// The storage panel: what is left, whether imports are free, and when it fills.
pub(super) fn storage(panel: &Panel<Storage>, room: usize) -> Vec<Line<'static>> {
    let storage = match panel {
        Panel::Ready(storage) => storage,
        Panel::Unavailable { reason } => return unavailable(reason, room),
    };
    let free = match &storage.free {
        Reading::Known(bytes) => Span::raw(format!("{} free", size(*bytes))),
        Reading::Stale(bytes) => Span::styled(format!("{} free ·", size(*bytes)), quiet()),
        // "The volume could not be read" and "the disk is full" are opposite
        // things to an operator and must never render alike.
        Reading::Unknown => Span::styled("free space could not be read".to_owned(), quiet()),
    };
    vec![
        Line::from(vec![free]),
        Line::from(vec![Span::raw(match storage.hardlink {
            Hardlink::Linking => "imports hardlink",
            // The consequence, not the property: what an operator needs to know is
            // that every import costs a second copy.
            Hardlink::Copying => "imports copy — twice the disk, and slower",
            Hardlink::Unknown => "it could not be established whether imports link",
        })]),
        Line::from(vec![storage.exhaustion.map_or_else(
            || Span::styled("not projected to fill".to_owned(), quiet()),
            |left| Span::raw(format!("full in ~{}", spell_out(left))),
        )]),
    ]
}

/// The services panel: what each one is doing.
pub(super) fn services(panel: &Panel<Vec<Service>>, room: usize) -> Vec<Line<'static>> {
    let services = match panel {
        Panel::Ready(services) => services,
        Panel::Unavailable { reason } => return unavailable(reason, room),
    };
    if services.is_empty() {
        return vec![Line::styled("no services are running", quiet())];
    }
    let mut lines: Vec<Line<'static>> = services
        .iter()
        .take(SHOWN)
        .map(|service| {
            let state = format!("{:?}", service.state).to_lowercase();
            let allowed = room.saturating_sub(state.chars().count());
            let column = LISTED.min(allowed);
            let named = shortened(&service.id, allowed);
            Line::from(vec![
                Span::raw(format!("{named:<column$}")),
                Span::raw(state),
            ])
        })
        .collect();
    lines.extend(rest(services.len(), "service"));
    lines
}

/// The line that stands for everything not shown, or nothing where it all was.
///
/// A count rather than silence: a truncated list that does not say it was
/// truncated is one an operator reads as complete.
fn rest(total: usize, noun: &str) -> Option<Line<'static>> {
    total
        .checked_sub(SHOWN)
        .filter(|more| *more > 0)
        .map(|more| {
            Line::styled(
                format!(
                    "and {more} more {noun}{} — {total} in all",
                    lemonfiber_core::plural::s(more)
                ),
                quiet(),
            )
        })
}

/// What an unavailable panel says: why, in the operator's terms.
///
/// One panel, one source. A panel that could not be filled marks itself and
/// leaves the rest of the screen live.
fn unavailable(reason: &str, room: usize) -> Vec<Line<'static>> {
    // The reason commonly quotes what a service said, which is text from
    // somewhere else on its way to a terminal.
    const LED: &str = "unavailable — ";
    vec![Line::styled(
        format!(
            "{LED}{}",
            shortened(reason, room.saturating_sub(LED.chars().count()))
        ),
        quiet(),
    )]
}

/// How many columns a line has taken so far.
fn width(line: &Line<'static>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

/// Whether any panel in this snapshot could not be filled.
///
/// What the header reads to say the screen is degraded rather than live — and it
/// is the panels themselves that decide it, so the two cannot disagree.
pub(super) fn any_panel_down(snapshot: &Snapshot) -> bool {
    !snapshot.transfers.is_available()
        || !snapshot.queue.is_available()
        || !snapshot.storage.is_available()
        || !snapshot.services.is_available()
        || snapshot.vpn.as_ref().is_some_and(|vpn| !vpn.is_available())
}

#[cfg(test)]
mod tests {
    use super::{
        alerts, any_panel_down, header, queues, services, storage, stuck, transfers, vpn, SHOWN,
    };
    use lemonfiber_core::alert::Alert;
    use lemonfiber_core::dashboard::{
        Hardlink, Panel, Protocol, Queue, Reading, Storage, Telemetry, Transfer, Vpn,
    };
    use lemonfiber_core::health::{Reach, Summary};
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
        assert!(lines.iter().any(|line| line.contains("NOT going through")));
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
            profile: "tv".to_owned(),
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

    #[test]
    fn a_pipeline_with_nothing_stuck_says_so() {
        assert_eq!(said(&stuck(&[], WIDE)), vec!["nothing is stuck".to_owned()]);
    }
}
