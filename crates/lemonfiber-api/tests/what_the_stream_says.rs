//! What one event looks like, and what a client that comes back is handed.
//!
//! Both are decisions rather than behaviours, so a gap here is something a test
//! makes rather than something it waits for.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lemonfiber_api::events::backlog::{Backlog, HELD};
use lemonfiber_api::events::wire::{Event, Nature, Rendered, BEAT, BEAT_SAID};
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::ports::Clock;
use lemonfiber_fixtures::ports::Stopped;
use serde::{Serialize, Serializer};

/// A clock reading before the epoch, which a run cannot be named for.
struct Backwards;

impl Clock for Backwards {
    fn now(&self) -> SystemTime {
        UNIX_EPOCH - Duration::from_secs(1)
    }
}

/// A payload that will not serialise, for the one answer rendering can give
/// besides an envelope.
struct Awkward;

impl Serialize for Awkward {
    fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("this one does not go on a wire"))
    }
}

/// A run of the stream that has said some things, and what it called each.
///
/// An envelope that would not render is not recorded, so the run is short one id
/// and every test that asks for it is handed nothing — which fails the test
/// rather than passing it on a stream that said less than it was asked to.
struct Run {
    backlog: Backlog,
    said: Vec<Event>,
}

impl Run {
    /// A run opened at a moment a test can write down.
    ///
    /// The mark is nanoseconds since the epoch in hex, so a run opened at the
    /// epoch itself calls its first event `0-1`.
    fn opened() -> Self {
        Self::by(Stopped::at(0).as_ref())
    }

    /// A run opened by a clock of the test's choosing.
    fn by(clock: &dyn Clock) -> Self {
        Self {
            backlog: Backlog::opening(clock),
            said: Vec::new(),
        }
    }

    /// The run, having also said this.
    fn said(mut self, nature: Nature, what: &'static str) -> Self {
        let kind = match nature {
            Nature::State => kind::DASHBOARD,
            Nature::Record => kind::LOG,
        };
        if let Some(said) = Rendered::of(nature, &Envelope::new(kind, what)) {
            let event = self.backlog.say(said);
            self.said.push(event);
        }
        self
    }

    /// What it called the nth thing it said.
    fn id(&self, nth: usize) -> Option<&str> {
        self.said.get(nth).map(Event::id)
    }

    /// The nth thing it said, as it goes down the wire.
    fn framed(&self, nth: usize) -> String {
        self.said.get(nth).map(Event::framed).unwrap_or_default()
    }

    /// What a client that last saw `seen` is handed.
    fn since(&self, seen: Option<&str>) -> Vec<Event> {
        self.backlog.since(seen)
    }

    /// What a client that last saw `seen` is handed, as it goes down the wire.
    fn wire(&self, seen: Option<&str>) -> String {
        self.since(seen).iter().map(Event::framed).collect()
    }
}

#[test]
fn a_silence_is_broken_at_fifteen_seconds() {
    assert_eq!(BEAT, Duration::from_secs(15));
}

#[test]
fn what_breaks_a_silence_is_a_comment_and_not_an_event() {
    assert!(BEAT_SAID.starts_with(':'), "{BEAT_SAID:?}");
    assert!(BEAT_SAID.ends_with("\n\n"), "{BEAT_SAID:?}");
    assert!(!BEAT_SAID.contains("event:"), "{BEAT_SAID:?}");
}

#[test]
fn a_payload_that_will_not_serialise_is_not_sent_as_something_else() {
    assert!(Rendered::of(Nature::State, &Envelope::new(kind::DASHBOARD, Awkward)).is_none());
}

#[test]
fn an_event_is_named_for_its_kind_and_carries_the_whole_envelope() {
    let wire = Run::opened().said(Nature::State, "one moment").framed(0);

    assert!(wire.contains("\nevent: dashboard\n"), "{wire}");
    assert!(wire.contains(r#""api_version":1"#), "{wire}");
    assert!(wire.contains(r#""kind":"dashboard""#), "{wire}");
    assert!(wire.contains(r#""data":"one moment""#), "{wire}");
}

#[test]
fn an_event_carries_an_id_a_client_can_come_back_with() {
    let run = Run::opened()
        .said(Nature::State, "one moment")
        .said(Nature::Record, "first");

    assert_eq!(run.id(0), Some("0-1"));
    assert_eq!(run.id(1), Some("0-2"));
}

#[test]
fn an_event_ends_where_the_next_one_begins() {
    let wire = Run::opened().said(Nature::Record, "first").framed(0);

    assert!(wire.starts_with("id: 0-1\n"), "{wire}");
    assert!(wire.ends_with("\n\n"), "{wire}");
    assert_eq!(wire.matches("data: ").count(), 1, "{wire}");
}

#[test]
fn a_run_is_named_for_the_moment_it_opened() {
    let run = Run::by(Stopped::at(1).as_ref()).said(Nature::Record, "first");

    assert_eq!(run.id(0), Some("3b9aca00-1"));
}

#[test]
fn a_run_that_cannot_be_dated_is_still_a_run_of_its_own() {
    let run = Run::by(&Backwards).said(Nature::Record, "first");

    assert_eq!(run.id(0), Some("0-1"));
}

#[test]
fn a_client_that_has_seen_nothing_is_handed_nothing() {
    let run = Run::opened().said(Nature::Record, "first");

    assert!(run.since(None).is_empty());
}

#[test]
fn a_client_is_handed_the_record_it_missed() {
    let run = Run::opened()
        .said(Nature::Record, "first")
        .said(Nature::Record, "second")
        .said(Nature::Record, "third");
    let wire = run.wire(run.id(0));

    assert_eq!(wire.matches("id: ").count(), 2, "{wire}");
    assert!(wire.contains("second"), "{wire}");
    assert!(wire.contains("third"), "{wire}");
}

#[test]
fn a_client_is_never_handed_back_a_figure_from_before_the_gap() {
    let run = Run::opened()
        .said(Nature::Record, "first")
        .said(Nature::State, "during the gap")
        .said(Nature::Record, "second")
        .said(Nature::State, "later in the gap");
    let handed = run.since(run.id(0));
    let wire = run.wire(run.id(0));

    assert!(
        handed.iter().all(|event| event.nature() == Nature::Record),
        "{wire}"
    );
    assert!(wire.contains("second"), "the record it missed: {wire}");
    assert!(
        !wire.contains("event: dashboard"),
        "and no moment of a stack it has not been told is over: {wire}"
    );
}

#[test]
fn a_client_that_has_seen_everything_is_handed_nothing() {
    let run = Run::opened()
        .said(Nature::Record, "first")
        .said(Nature::Record, "second");

    assert!(run.since(run.id(1)).is_empty());
}

#[test]
fn a_run_that_has_said_nothing_hands_back_nothing() {
    assert!(Run::opened().since(Some("0-0")).is_empty());
}

#[test]
fn a_gap_wider_than_the_backlog_restarts_rather_than_handing_back_part_of_it() {
    let mut run = Run::opened().said(Nature::Record, "first");
    for _ in 0..=HELD {
        run = run.said(Nature::Record, "since");
    }

    assert!(
        run.since(run.id(0)).is_empty(),
        "what it missed is no longer whole"
    );
}

/// A gather every tick must not push out the records a client came back for.
///
/// The bound is on what is kept, and a state event is the one thing that is never
/// handed back. Keeping them spent the whole backlog on a few minutes of ticks:
/// a client that missed one log line while the dashboard gathered was told the
/// record was no longer whole, and handed nothing, while the line it wanted was
/// still there to give.
#[test]
fn a_quiet_stretch_of_gathers_does_not_cost_a_client_the_record_it_missed() {
    let mut run = Run::opened().said(Nature::Record, "before the gap");
    for _ in 0..=HELD {
        run = run.said(Nature::State, "another gather");
    }
    run = run.said(Nature::Record, "after the gap");

    let handed = run.since(run.id(0));
    assert_eq!(
        handed.len(),
        1,
        "the record said after the gathers is still there to hand back"
    );
    assert!(handed.iter().all(|event| event.nature() == Nature::Record));
}

#[test]
fn a_client_from_another_run_is_handed_nothing() {
    let run = Run::opened()
        .said(Nature::Record, "first")
        .said(Nature::Record, "second");

    assert!(
        run.since(Some("11abcd-1")).is_empty(),
        "its numbering belongs to a run that has ended"
    );
}

#[test]
fn a_client_naming_a_place_this_run_has_not_reached_is_handed_nothing() {
    let run = Run::opened().said(Nature::Record, "first");

    assert!(run.since(Some("0-9")).is_empty());
}

#[test]
fn a_client_naming_no_place_at_all_is_handed_nothing() {
    let run = Run::opened().said(Nature::Record, "first");

    assert!(run.since(Some("nonsense")).is_empty(), "no place in it");
    assert!(run.since(Some("0-later")).is_empty(), "not a number");
}
