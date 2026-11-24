//! What the stream can still hand back to a client that comes again.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use lemonfiber_core::ports::Clock;

use super::wire::{Event, Item, Rendered, Replayed};

/// How many records a run keeps to hand back.
pub const HELD: usize = 256;

/// How many items a run keeps the newest line of to hand back.
pub const ITEMS: usize = 256;

/// Everything one run of the stream has said, as far back as it still keeps it.
pub struct Backlog {
    /// What this run calls itself, the first half of every id it gives.
    run: String,
    /// The records said, oldest first, and where each was said.
    held: VecDeque<(u64, Event)>,
    /// The newest line of each item, and where it was said.
    lines: Vec<Line>,
    /// The latest place said of anything no longer kept.
    dropped: u64,
    /// The place the next thing said will take.
    next: u64,
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
            next: 1,
        }
    }

    /// Record one thing said, and give it its place in the run.
    pub fn say(&mut self, said: Rendered) -> Event {
        let place = self.next;
        self.next = self.next.saturating_add(1);
        let replayed = said.replayed();
        let (item, said) = said.parted();
        let event = Event::placed(format!("{}-{place}", self.run), said);
        match replayed {
            Replayed::Every => self.record(place, &event),
            Replayed::NewestOfEach => self.line(item, place, &event),
            Replayed::Never => {}
        }
        event
    }

    /// What a client that last saw `seen` is handed before the stream goes on.
    #[must_use]
    pub fn since(&self, seen: Option<&str>) -> Vec<Event> {
        let Some(place) = seen.and_then(|seen| self.place_of(seen)) else {
            return Vec::new();
        };
        if !self.whole_after(place) {
            return Vec::new();
        }
        let mut missed: Vec<(u64, &Event)> = self
            .held
            .iter()
            .map(|(at, event)| (*at, event))
            .chain(self.lines.iter().map(|line| (line.at, &line.event)))
            .filter(|(at, _)| *at > place)
            .collect();
        missed.sort_unstable_by_key(|(at, _)| *at);
        missed.into_iter().map(|(_, event)| event.clone()).collect()
    }

    /// Keep a record, letting the oldest go past [`HELD`].
    fn record(&mut self, place: u64, event: &Event) {
        self.held.push_back((place, event.clone()));
        if self.held.len() > HELD {
            if let Some((gone, _)) = self.held.pop_front() {
                self.dropped = self.dropped.max(gone);
            }
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

/// What a run calls itself: the moment it opened, or zero before the epoch.
fn mark(now: SystemTime) -> String {
    let opened = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!("{opened:x}")
}
