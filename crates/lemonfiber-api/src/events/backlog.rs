//! What the stream can still hand back to a client that comes again.

use std::collections::VecDeque;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lemonfiber_core::ports::Clock;
use schemars::JsonSchema;
use serde::Serialize;

use super::wire::{Event, Item, Rendered, Replayed};

/// How many records a run keeps to hand back.
pub const HELD: usize = 256;

/// How many items a run keeps the newest line of to hand back.
pub const ITEMS: usize = 256;

/// How many alerts a run keeps to hand back.
pub const ALERTS: usize = 200;

/// How long a run keeps an alert to hand back.
pub const ALERTS_FOR: Duration = Duration::from_secs(24 * 60 * 60);

/// What a client that comes again is handed before the stream goes on.
#[derive(Debug, Default)]
pub struct Handed {
    /// What it missed, in the order it was said.
    pub missed: Vec<Event>,
    /// What is said of the alerts among it, where it gave an id to come again from.
    pub replay: Option<Replay>,
}

/// What a stream opened again from an id says of what it is handed before going on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Replay {
    /// Whether it holds every alert raised since the id.
    pub alerts: ReplayAlerts,
}

/// Whether a replay holds every alert raised since the id it was asked from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReplayAlerts {
    /// Every one.
    Whole,
    /// Those still kept, and some raised since are not.
    Partial,
}

/// Everything one run of the stream has said, as far back as it still keeps it.
pub struct Backlog {
    /// What this run calls itself, the first half of every id it gives.
    run: String,
    /// The records said, oldest first, and where each was said: an alert by its
    /// place alone, the alerts being kept in [`raised`](Self::raised).
    held: VecDeque<(u64, Option<Event>)>,
    /// The newest line of each item, and where it was said.
    lines: Vec<Line>,
    /// The latest place said of anything no longer kept.
    dropped: u64,
    /// The recent alerts, oldest first.
    raised: VecDeque<Raised>,
    /// The latest place said of an alert no longer kept.
    forgotten: u64,
    /// The place the next thing said will take.
    next: u64,
}

/// One alert kept, where it was said and when.
struct Raised {
    /// Where it was said.
    at: u64,
    /// When it was said.
    when: SystemTime,
    /// What was said.
    event: Event,
}

/// The newest line said about one item.
struct Line {
    /// What it is about.
    item: Option<Item>,
    /// Where it was said.
    at: u64,
    /// What was said.
    event: Event,
}

impl Backlog {
    /// A backlog for a run opening now.
    #[must_use]
    pub fn opening(clock: &dyn Clock) -> Self {
        Self {
            run: mark(clock.now()),
            held: VecDeque::new(),
            lines: Vec::new(),
            dropped: 0,
            raised: VecDeque::new(),
            forgotten: 0,
            next: 1,
        }
    }

    /// Record one thing said at `now`, and give it its place in the run.
    pub fn say(&mut self, said: Rendered, now: SystemTime) -> Event {
        let place = self.next;
        self.next = self.next.saturating_add(1);
        let replayed = said.replayed();
        let (item, said) = said.parted();
        let event = Event::placed(format!("{}-{place}", self.run), said);
        match replayed {
            Replayed::Every => self.record(place, Some(&event)),
            Replayed::Recent => {
                self.record(place, None);
                self.raise(place, now, &event);
            }
            Replayed::NewestOfEach => self.line(item, place, &event),
            Replayed::Never => {}
        }
        event
    }

    /// What a client that last saw `seen` is handed at `now` before the stream goes on.
    #[must_use]
    pub fn since(&self, seen: Option<&str>, now: SystemTime) -> Handed {
        let Some(seen) = seen else {
            return Handed::default();
        };
        let place = self.place_of(seen);
        let whole = place.filter(|place| self.whole_after(*place));
        let forgotten = self
            .raised
            .iter()
            .filter(|raised| aged(raised.when, now))
            .map(|raised| raised.at)
            .fold(self.forgotten, u64::max);
        let records = self
            .held
            .iter()
            .filter_map(|(at, event)| Some((*at, event.as_ref()?)))
            .chain(self.lines.iter().map(|line| (line.at, &line.event)))
            .filter(|(at, _)| whole.is_some_and(|place| *at > place));
        let alerts = self
            .raised
            .iter()
            .filter(|raised| !aged(raised.when, now))
            .map(|raised| (raised.at, &raised.event))
            .filter(|(at, _)| place.is_none_or(|place| *at > place));
        let mut missed: Vec<(u64, &Event)> = records.chain(alerts).collect();
        missed.sort_unstable_by_key(|(at, _)| *at);
        Handed {
            missed: missed.into_iter().map(|(_, event)| event.clone()).collect(),
            replay: Some(Replay {
                alerts: match place {
                    Some(place) if forgotten <= place => ReplayAlerts::Whole,
                    _ => ReplayAlerts::Partial,
                },
            }),
        }
    }

    /// Keep a record, letting the oldest go past [`HELD`].
    fn record(&mut self, place: u64, event: Option<&Event>) {
        self.held.push_back((place, event.cloned()));
        if self.held.len() > HELD {
            if let Some((gone, _)) = self.held.pop_front() {
                self.dropped = self.dropped.max(gone);
            }
        }
    }

    /// Keep an alert, letting the oldest go past [`ALERTS`]; one older than
    /// [`ALERTS_FOR`] is passed over where it is handed back.
    fn raise(&mut self, place: u64, now: SystemTime, event: &Event) {
        self.raised.push_back(Raised {
            at: place,
            when: now,
            event: event.clone(),
        });
        if self.raised.len() > ALERTS {
            self.forget_oldest();
        }
    }

    /// Let go of the oldest alert kept.
    fn forget_oldest(&mut self) {
        if let Some(gone) = self.raised.pop_front() {
            self.forgotten = self.forgotten.max(gone.at);
        }
    }

    /// Keep a line as its item's newest, letting the item whose newest is oldest go
    /// past [`ITEMS`].
    fn line(&mut self, item: Option<Item>, place: u64, event: &Event) {
        if let Some(line) = self.lines.iter_mut().find(|line| line.item == item) {
            line.at = place;
            line.event = event.clone();
            return;
        }
        if self.lines.len() >= ITEMS {
            let oldest = self
                .lines
                .iter()
                .enumerate()
                .min_by_key(|(_, line)| line.at)
                .map(|(nth, _)| nth);
            if let Some(oldest) = oldest {
                let gone = self.lines.swap_remove(oldest);
                self.dropped = self.dropped.max(gone.at);
            }
        }
        self.lines.push(Line {
            item,
            at: place,
            event: event.clone(),
        });
    }

    /// Whether everything said after `place` that is ever handed back is still kept.
    const fn whole_after(&self, place: u64) -> bool {
        self.dropped <= place
    }

    /// Where in this run a client's last id puts it, where it is one of this
    /// run's and one this run has reached.
    fn place_of(&self, seen: &str) -> Option<u64> {
        let (run, place) = seen.rsplit_once('-')?;
        if run != self.run {
            return None;
        }
        place.parse::<u64>().ok().filter(|place| *place < self.next)
    }
}

/// Whether something said at `when` was said longer than [`ALERTS_FOR`] before `now`.
fn aged(when: SystemTime, now: SystemTime) -> bool {
    now.duration_since(when)
        .is_ok_and(|since| since > ALERTS_FOR)
}

/// What a run calls itself: the moment it opened, or zero before the epoch.
fn mark(now: SystemTime) -> String {
    let opened = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!("{opened:x}")
}
