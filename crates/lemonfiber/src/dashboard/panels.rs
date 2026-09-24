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

use crate::pane::quiet;
use crate::render::door::standing;
use crate::render::downloads::protocol;
use lemonfiber_core::alert::Alert;
use lemonfiber_core::dashboard::{
    Hardlink, Panel, Queue, Reading, Snapshot, Storage, Telemetry, Transfer, Vpn,
};
use lemonfiber_core::docker::Service;
use lemonfiber_core::health::Summary;
use lemonfiber_core::household::State;
use lemonfiber_core::model::{FrontDoorReport, HouseholdReport};
use lemonfiber_core::queue::Stuck;
use lemonfiber_core::text::{fitted, plain};
use lemonfiber_core::walkthrough::{size, spell_out};
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
                Span::raw("the client's traffic is NOT inside the tunnel")
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
    let mut lines: Vec<Line<'static>> = alerts
        .iter()
        .take(SHOWN)
        .map(|alert| {
            let moment = format!("{:<9}", alert.moment.said());
            let left = room.saturating_sub(moment.chars().count());
            Line::from(vec![
                Span::raw(moment),
                Span::raw(shortened(&alert.summary, left)),
            ])
        })
        .collect();
    lines.extend(rest(alerts.len(), "alert"));
    lines
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

/// The front door: the one address to hand somebody who lives here.
///
/// On the screen rather than behind a question, because the operator who needs it is
/// not the one who thought to ask — they have just been asked "what do I open?" by
/// somebody in the next room, and a screen that already has the answer should not
/// make them go and fetch it.
///
/// The address is its own line and the whole of it is shown, because an address is
/// only worth having if it can be read out; the name and the phrase beside it share
/// the line above, which is where the room is spent when there is not enough.
pub(super) fn front_door(panel: &Panel<FrontDoorReport>, room: usize) -> Vec<Line<'static>> {
    let report = match panel {
        Panel::Ready(report) => report,
        Panel::Unavailable { reason } => return unavailable(reason, room),
    };
    let phrase = standing(report.standing);
    let mut lines = vec![match &report.service {
        Some(service) => Line::from(vec![
            Span::raw(shortened(
                service,
                room.saturating_sub(phrase.chars().count() + 2),
            )),
            Span::raw("  "),
            Span::styled(phrase.to_owned(), quiet()),
        ]),
        None => Line::styled(phrase.to_owned(), quiet()),
    }];
    if let Some(address) = &report.address {
        lines.push(Line::raw(shortened(&address.url, room)));
    }
    // What the operator's own file did to this answer, where it did anything. A
    // refused setting that only appeared in the long answer would be one an operator
    // watching this screen never learned about.
    if let Some(said) = report.chosen.said() {
        lines.push(Line::styled(shortened(&said, room), quiet()));
    }
    lines
}

/// The household panel: what somebody has asked for that is not moving.
///
/// Only the two states an operator can act on — waiting for a decision, and failed
/// after approval. A request that is being fetched or is already here needs nobody,
/// and listing it would push the ones that do off a panel this size.
pub(super) fn household(panel: &Panel<HouseholdReport>, room: usize) -> Vec<Line<'static>> {
    let report = match panel {
        Panel::Ready(report) => report,
        Panel::Unavailable { reason } => return unavailable(reason, room),
    };
    if !report.available {
        // The household is unread when the *media server* would not say who holds an
        // account. A request service that refused costs the requests and not the
        // household, and says so in a finding beside a list that still reads.
        return vec![Line::styled("the household was not read", quiet())];
    }
    let waiting: Vec<Line<'static>> = report
        .members
        .iter()
        .flat_map(|member| {
            member
                .requests
                .iter()
                .filter(|request| wants_the_operator(request.state))
                .map(|request| line(&member.name, request.title.as_deref(), request.state, room))
        })
        .collect();
    if waiting.is_empty() {
        return vec![Line::styled("nothing is waiting on you", quiet())];
    }
    let total = waiting.len();
    let mut lines: Vec<Line<'static>> = waiting.into_iter().take(SHOWN).collect();
    lines.extend(rest(total, "request"));
    lines
}

/// Whether a request is one the operator has to do something about.
const fn wants_the_operator(state: Option<State>) -> bool {
    matches!(state, Some(State::WaitingForApproval | State::Failed))
}

/// One request: who asked, what for, and where it stands.
fn line(who: &str, title: Option<&str>, state: Option<State>, room: usize) -> Line<'static> {
    let standing = match state {
        Some(State::Failed) => "failed",
        _ => "waiting",
    };
    let asked = title.unwrap_or("not named yet");
    let said = format!("{who}: {asked}");
    Line::from(vec![
        Span::raw(shortened(
            &said,
            room.saturating_sub(standing.chars().count() + 2),
        )),
        Span::raw("  "),
        Span::styled(standing.to_owned(), quiet()),
    ])
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
        || !snapshot.door.is_available()
        || !snapshot.household.is_available()
        || snapshot.vpn.as_ref().is_some_and(|vpn| !vpn.is_available())
}

#[cfg(test)]
mod tests;
