//! What one event looks like on the wire, and what a silence says instead.
//!
//! No payload is serialised here either. An envelope renders itself, and that
//! rendering is the one the command line prints, so the stream and the command
//! cannot describe the same moment differently.

use std::time::Duration;

use lemonfiber_core::model::kind::Kind;
use lemonfiber_core::model::Envelope;
use serde::Serialize;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nature {
    /// The whole of some state at one moment. Only the newest describes now, so
    /// an older one is never handed to a client that missed it — it would be a
    /// value from before the gap arriving as though it were current.
    State,
    /// One thing that happened. Skipping it leaves a hole in the record rather
    /// than an out-of-date figure, so a client that missed it is given it.
    Record,
}

/// Who on the shared stream may hear an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    /// Everyone the shared stream admits, a `read` or an `act` key included.
    Anyone,
    /// The per-run token and an operator session, and no key.
    Operator,
}

impl Audience {
    /// What this caller may hear on the shared stream.
    #[must_use]
    pub const fn of(caller: &crate::admission::Caller) -> Self {
        match caller {
            crate::admission::Caller::Machine | crate::admission::Caller::Operator => {
                Self::Operator
            }
            crate::admission::Caller::Member(_) | crate::admission::Caller::Key(_) => Self::Anyone,
        }
    }

    /// Whether a listener hearing as this may hear what is said to `said`.
    #[must_use]
    pub const fn hears(self, said: Self) -> bool {
        matches!(
            (self, said),
            (Self::Operator, _) | (Self::Anyone, Self::Anyone)
        )
    }
}

/// An envelope rendered, before it has a place in the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// What the envelope calls itself, which is the name the event carries.
    kind: Kind,
    /// The envelope, as the command line renders it.
    said: String,
    /// Whether a newer one of this kind replaces it.
    nature: Nature,
    /// Who may hear it.
    audience: Audience,
}

impl Rendered {
    /// Render an envelope for the stream.
    ///
    /// `None` where the payload will not serialise, which for the values this
    /// carries cannot happen — an event that cannot be rendered is not sent
    /// rather than sent as something else.
    #[must_use]
    pub fn of<T: Serialize>(nature: Nature, envelope: &Envelope<T>) -> Option<Self> {
        Some(Self {
            kind: envelope.kind,
            said: envelope.to_json()?,
            nature,
            audience: Audience::Anyone,
        })
    }

    /// The same envelope, said to the operator alone.
    #[must_use]
    pub fn to_the_operator(self) -> Self {
        Self {
            audience: Audience::Operator,
            ..self
        }
    }

    /// What the envelope calls itself.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
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
        self.said.nature
    }

    /// Who may hear it.
    #[must_use]
    pub const fn audience(&self) -> Audience {
        self.said.audience
    }

    /// This event as it goes down the wire.
    ///
    /// The payload occupies as many `data` lines as it has lines, which a client
    /// rejoins with the newlines it split on. Rendered JSON has none of its own —
    /// a newline inside a value is escaped — so in practice this is one line.
    #[must_use]
    pub fn framed(&self) -> String {
        let mut wire = format!("id: {}\nevent: {}\n", self.id, self.said.kind);
        for line in self.said.said.lines() {
            wire.push_str("data: ");
            wire.push_str(line);
            wire.push('\n');
        }
        wire.push('\n');
        wire
    }
}
