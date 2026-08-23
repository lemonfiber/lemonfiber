//! What the stream can still hand back to a client that comes again.
//!
//! A client that reconnects says where it got to, and there are two honest
//! answers: the record it missed, or nothing at all. There is no third answer in
//! which a figure gathered before the gap is handed back as though it were
//! current — so state events are never retransmitted, however recent. What is
//! current comes from a gather made after the client returned, and nothing else.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use lemonfiber_core::ports::Clock;

use super::wire::{Event, Nature, Rendered};

/// How many records a client may have missed and still be told what they were.
///
/// Bounded because a stream nobody is listening to must not grow: past this the
/// record is no longer whole, and the stream restarts rather than handing back a
/// part of it.
///
/// Records, because records are what can be handed back. A state event is
/// gathered every tick and never retransmitted, so keeping one costs a whole
/// snapshot to hold something nobody can ever be given — and, because the bound
/// counts what is kept, it pushes out the records that could have been.
pub const HELD: usize = 256;

/// Everything one run of the stream has said, as far back as it still keeps it.
pub struct Backlog {
    /// What this run calls itself.
    ///
    /// Ids are a run's own counting, so the same number names a different event
    /// in the next run. Carrying the run in the id is what lets a client that
    /// returns to a restarted server be recognised as such, rather than told it
    /// is up to date because its number happens to be one this run has reached.
    run: String,
    /// The records said, oldest first, and where each was said.
    held: VecDeque<(u64, Event)>,
    /// The place the next thing said will take.
    next: u64,
}

impl Backlog {
    /// A backlog for a run opening now.
    #[must_use]
    pub fn opening(clock: &dyn Clock) -> Self {
        Self {
            run: mark(clock.now()),
            held: VecDeque::new(),
            next: 1,
        }
    }

    /// Record one thing said, and give it its place in the run.
    ///
    /// Everything said takes a place, because the place is what a client names
    /// when it comes back. Only a record is kept: a state event is the one thing
    /// [`since`](Self::since) will never hand back, so keeping one holds a whole
    /// snapshot for nothing and spends a slot a record could have had.
    pub fn say(&mut self, said: Rendered) -> Event {
        let place = self.next;
        let event = Event::placed(format!("{}-{place}", self.run), said);
        self.next = self.next.saturating_add(1);
        if event.nature() == Nature::Record {
            self.held.push_back((place, event.clone()));
            while self.held.len() > HELD {
                self.held.pop_front();
            }
        }
        event
    }

    /// What a client that last saw `seen` is handed before the stream goes on.
    ///
    /// The record it missed, where the whole of that record is still held. An id
    /// from another run, an id this run has not reached, and a gap reaching
    /// further back than the backlog all mean the same thing: the record cannot
    /// be completed, so the stream restarts and hands back nothing.
    #[must_use]
    pub fn since(&self, seen: Option<&str>) -> Vec<Event> {
        let Some(place) = seen.and_then(|seen| self.place_of(seen)) else {
            return Vec::new();
        };
        let covered = self
            .held
            .front()
            .is_none_or(|(oldest, _)| *oldest <= place.saturating_add(1));
        if !covered {
            return Vec::new();
        }
        self.held
            .iter()
            .filter(|(at, event)| *at > place && event.nature() == Nature::Record)
            .map(|(_, event)| event.clone())
            .collect()
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

/// What a run calls itself: the moment it opened.
///
/// A clock that reads before the epoch gives a run no moment to be named for,
/// and it is named zero. Two runs on such a machine are then not told apart,
/// which costs a returning client one restart rather than anything it is shown.
fn mark(now: SystemTime) -> String {
    let opened = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!("{opened:x}")
}
