//! What one event looks like on the wire, and what a silence says instead.
//!
//! No payload is serialised here either. An envelope renders itself, and that
//! rendering is the one the command line prints, so the stream and the command
//! cannot describe the same moment differently.

use std::time::Duration;

use lemonfiber_core::alert::Alert;
use lemonfiber_core::copying::Copying;
use lemonfiber_core::dashboard::Snapshot;
use lemonfiber_core::keys::Scope;
use lemonfiber_core::logs::Line as LogLine;
use lemonfiber_core::model::kind::{self, Kind};
use lemonfiber_core::model::{
    Envelope, HeldReport, HouseholdReport, PlayingReport, SettingsReport, WiringReport,
};
use lemonfiber_core::news::Newest;
use lemonfiber_core::plugin::Installs;
use lemonfiber_core::walkthrough::Line as StepLine;
use schemars::{schema_for, Schema};
use serde::Serialize;

use super::backlog::Replay;

/// The longest the stream may say nothing before saying it is still there.
///
/// A client treats twice this in silence as a broken connection, which tolerates
/// one missed beat without mistaking a dead connection for a quiet one.
pub const BEAT: Duration = Duration::from_secs(15);

/// What a silence is broken with.
///
/// A comment line: every client discards it, so nothing has to know a name for
/// it, and none can mistake it for something that happened.
pub const BEAT_SAID: &str = ": beat\n\n";

/// Whether a newer event of the same kind leaves this one worth having.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Nature {
    /// The whole of some state at one moment. Only the newest describes now, so
    /// an older one is never handed to a client that missed it — it would be a
    /// value from before the gap arriving as though it were current.
    State,
    /// One thing that happened. Skipping it leaves a hole in the record rather
    /// than an out-of-date figure, so a client that missed it is given it.
    Record,
}

/// What a client that resumes is handed again of one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replayed {
    /// Every one the run still keeps.
    Every,
    /// Every one said lately that the run still keeps, handed back even where the rest is not.
    Recent,
    /// The newest of each item the run still keeps.
    NewestOfEach,
    /// None.
    Never,
}

/// What a `start` or `pull` line is about: the work its job names, and for a pull the
/// image or layer its first word names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    /// Whether it is a start or a pull line.
    event: EventKind,
    /// The job that said it.
    job: Option<String>,
    /// The image or layer a pull line names, empty for a start line.
    about: String,
}

impl Item {
    /// What `line`, said as `event` by the work `job` names, is about.
    fn of(event: EventKind, line: &str, job: Option<&str>) -> Self {
        let about = match event {
            EventKind::Pull => line.split_whitespace().next().unwrap_or_default(),
            _ => "",
        };
        Self {
            event,
            job: job.map(str::to_owned),
            about: about.to_owned(),
        }
    }
}

/// Who on the shared stream may hear an event, and who a listener hears as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Audience {
    /// Everyone the shared stream admits, a `read` or an `act` key included; as a listener,
    /// one that proved neither the operator nor such a key.
    Anyone,
    /// The per-run token and an operator session, and no key.
    Operator,
    /// A `read` or an `act` key, and not the operator.
    Key,
    /// The operator and a `read` or an `act` key.
    OperatorAndKeys,
}

impl Audience {
    /// What this caller may hear on the shared stream.
    #[must_use]
    pub const fn of(caller: &crate::admission::Caller) -> Self {
        match caller {
            crate::admission::Caller::Machine | crate::admission::Caller::Operator => {
                Self::Operator
            }
            crate::admission::Caller::Key(crate::admission::Keyed {
                scope: Scope::Read | Scope::Act,
                ..
            }) => Self::Key,
            crate::admission::Caller::Member(_) | crate::admission::Caller::Key(_) => Self::Anyone,
        }
    }

    /// Whether a listener hearing as this may hear what is said to `said`.
    #[must_use]
    pub const fn hears(self, said: Self) -> bool {
        matches!(
            (self, said),
            (_, Self::Anyone)
                | (Self::Operator, Self::Operator | Self::OperatorAndKeys)
                | (Self::Key, Self::Key | Self::OperatorAndKeys)
        )
    }
}

/// Declares [`EventKind`] from one list of each kind, its payload, its nature and its audience.
macro_rules! event_kinds {
    ($($(#[doc = $doc:literal])* $variant:ident($payload:ty) = $kind:ident, $nature:ident, $audience:ident;)*) => {
        /// Every kind of event the stream may say, and nothing else.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum EventKind {
            $($(#[doc = $doc])* $variant,)*
        }

        impl EventKind {
            /// Every kind of event, in the order the list declares them.
            pub const ALL: &[Self] = &[$(Self::$variant,)*];

            /// The kind the event's envelope carries, which is the name the event goes by.
            #[must_use]
            pub const fn kind(self) -> Kind {
                match self {
                    $(Self::$variant => kind::$kind,)*
                }
            }

            /// Whether a newer event of this kind replaces an older one.
            #[must_use]
            pub const fn nature(self) -> Nature {
                match self {
                    $(Self::$variant => Nature::$nature,)*
                }
            }

            /// What a client that resumes is handed again of this kind.
            #[must_use]
            pub const fn replayed(self) -> Replayed {
                match (self, self.nature()) {
                    (Self::Start | Self::Pull, _) => Replayed::NewestOfEach,
                    (Self::Alert, _) => Replayed::Recent,
                    (_, Nature::Record) => Replayed::Every,
                    (_, Nature::State) => Replayed::Never,
                }
            }

            /// Who on the shared stream may hear an event of this kind.
            #[must_use]
            pub const fn audience(self) -> Audience {
                match self {
                    $(Self::$variant => Audience::$audience,)*
                }
            }

            /// The schema of the envelope an event of this kind is said in.
            #[must_use]
            pub fn schema(self) -> Schema {
                match self {
                    $(Self::$variant => schema_for!(Envelope<$payload>),)*
                }
            }
        }
    };
}

event_kinds! {
    /// One alert, at its onset or its resolution.
    Alert(Alert) = ALERT, Record, Anyone;
    /// The backup or restore running on this machine now, or none.
    Copying(Copying) = COPYING, State, OperatorAndKeys;
    /// One moment of the dashboard.
    Dashboard(Snapshot) = DASHBOARD, State, Anyone;
    /// What one member can watch.
    Held(HeldReport) = HELD, State, Anyone;
    /// One member's household row.
    Household(HouseholdReport) = HOUSEHOLD, State, Anyone;
    /// One line a followed service wrote.
    Log(LogLine) = LOG, Record, Anyone;
    /// The newest few of each kind a surface can mark as new.
    News(Newest) = NEWS, State, Anyone;
    /// What the media server is playing.
    Playing(PlayingReport) = PLAYING, State, Anyone;
    /// Every plugin installed.
    Plugins(Installs) = PLUGINS, State, Anyone;
    /// One line the container engine wrote while pulling images.
    Pull(String) = PULL, State, Anyone;
    /// Whether what a stream opened again hands back holds every alert raised since.
    Replay(Replay) = REPLAY, State, Anyone;
    /// Every setting.
    Settings(SettingsReport) = SETTINGS, State, Operator;
    /// One line said while services are starting.
    Start(String) = START, State, Anyone;
    /// One step of a walkthrough.
    Step(StepLine) = STEP, Record, Anyone;
    /// What the stack wires to what.
    Wiring(WiringReport) = WIRING, State, Anyone;
}

/// An envelope rendered, before it has a place in the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// What kind of event it is.
    event: EventKind,
    /// The envelope, as the command line renders it.
    said: String,
    /// What it is about, where it is a line kept newest of each.
    item: Option<Item>,
    /// Who on the shared stream may hear it.
    to: Audience,
}

impl Rendered {
    /// Render a payload for the stream as an event of one kind.
    ///
    /// `None` where the payload will not serialise, which for the values this
    /// carries cannot happen — an event that cannot be rendered is not sent
    /// rather than sent as something else.
    #[must_use]
    pub fn of<T: Serialize>(event: EventKind, payload: &T) -> Option<Self> {
        Self::said_by(event, payload, None)
    }

    /// Render a payload for the stream as an event of one kind, said by the work a job names.
    #[must_use]
    pub fn said_by<T: Serialize>(event: EventKind, payload: &T, job: Option<&str>) -> Option<Self> {
        Some(Self {
            event,
            said: Envelope::new(event.kind(), payload)
                .said_by(job)
                .to_json()?,
            item: None,
            to: event.audience(),
        })
    }

    /// The same event, said to `audience` alone.
    #[must_use]
    pub fn to(mut self, audience: Audience) -> Self {
        self.to = audience;
        self
    }

    /// Render one line for the stream as an event of one kind, said by the work a job names.
    #[must_use]
    pub fn line(event: EventKind, line: &str, job: Option<&str>) -> Option<Self> {
        Some(Self {
            item: Some(Item::of(event, line, job)),
            ..Self::said_by(event, &line, job)?
        })
    }

    /// What a client that resumes is handed again of its kind.
    pub(crate) const fn replayed(&self) -> Replayed {
        self.event.replayed()
    }

    /// What it is about, apart from the rest of it.
    pub(crate) fn parted(mut self) -> (Option<Item>, Self) {
        (self.item.take(), self)
    }

    /// What the envelope calls itself.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.event.kind()
    }

    /// Who on the shared stream may hear it.
    #[must_use]
    pub const fn audience(&self) -> Audience {
        self.to
    }

    /// The envelope, as the command line renders it.
    #[must_use]
    pub fn said(&self) -> &str {
        &self.said
    }
}

/// One thing the stream said, and where in the run it said it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// What a client sends back to say this is where it got to.
    id: String,
    /// What was said.
    said: Rendered,
}

impl Event {
    /// One rendered envelope, given its place in the run.
    pub(crate) const fn placed(id: String, said: Rendered) -> Self {
        Self { id, said }
    }

    /// Where in the run this was said.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Whether a newer one of this kind replaces it.
    #[must_use]
    pub const fn nature(&self) -> Nature {
        self.said.event.nature()
    }

    /// Who may hear it.
    #[must_use]
    pub const fn audience(&self) -> Audience {
        self.said.audience()
    }

    /// This event as it goes down the wire.
    #[must_use]
    pub fn framed(&self) -> String {
        framed(Some(&self.id), &self.said)
    }
}

/// An envelope as it goes down the wire, under `id` where it has a place in the run.
///
/// The payload occupies as many `data` lines as it has lines, which a client
/// rejoins with the newlines it split on. Rendered JSON has none of its own —
/// a newline inside a value is escaped — so in practice this is one line.
pub(crate) fn framed(id: Option<&str>, said: &Rendered) -> String {
    let mut wire = String::new();
    if let Some(id) = id {
        wire.push_str("id: ");
        wire.push_str(id);
        wire.push('\n');
    }
    wire.push_str("event: ");
    wire.push_str(said.kind().as_str());
    wire.push('\n');
    for line in said.said.lines() {
        wire.push_str("data: ");
        wire.push_str(line);
        wire.push('\n');
    }
    wire.push('\n');
    wire
}
