//! What a household member hears on a stream of their own.
//!
//! The operator's stream is the dashboard, the log lines and what setup is doing, and
//! none of it is narrowed to anybody, so a member is never handed it. A member's
//! stream carries three things and nothing else: their household row, their shelf and
//! what they are playing, each the answer the read gives that member.
//!
//! **Narrowed by the same functions every member's read is.** Each reading is the
//! command [`crate::entitled`] gives a member who asks for it themselves, built by the
//! one function that decides what that read narrows to, so the stream cannot say more
//! than the member's own reads would — not a second copy of that answer written here.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::model::kind::Kind;
use lemonfiber_core::model::{HeldReport, HouseholdReport, PlayingReport};
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::live::Gathers;
use super::wire::{Nature, Rendered};
use crate::admission::Caller;
use crate::entitled::{their_household, their_playing, their_shelf};
use crate::read::table::A_SHELF;

/// How often what a member is playing is read again while they listen.
///
/// Slower than the dashboard's tick, because each reading is a question to the media
/// server on one person's behalf, and fast enough that a film started is heard about
/// before anybody wonders whether it was.
pub const PLAYING_EVERY: Duration = Duration::from_secs(5);

/// How often a member's household row and shelf are read again while they listen.
///
/// They move when somebody asks for something or something arrives, which is minutes
/// rather than seconds, and the shelf is the largest of the three answers.
pub const ROWS_EVERY: Duration = Duration::from_secs(60);

/// The finding a reading carries where the stack itself could not be read.
///
/// Said in a sentence of its own rather than as the problem the command reported,
/// because that problem is written for the operator and names what only the operator
/// is shown.
const UNREAD: &str = "the stack could not be read just now, so this could not be said";

/// One member's stream: what to ask, for whom, and what was last said.
pub struct Theirs {
    /// The world a member's reads run against.
    ctx: Arc<Ctx>,
    /// The member's id, which is what makes this stream theirs.
    member: String,
    /// When each pace last came round, and what was last said of each kind.
    heard: Mutex<Heard>,
}

/// When each reading was last made, and what it said.
#[derive(Default)]
struct Heard {
    /// When what is playing was last read.
    playing: Option<Instant>,
    /// When the household row and the shelf were last read.
    rows: Option<Instant>,
    /// The last envelope said of each kind, so an unchanged one is not said again.
    said: BTreeMap<Kind, String>,
}

impl Theirs {
    /// A stream for the member a caller acts as, or none for a caller who is not one.
    ///
    /// The commands are only a member's once the decision has narrowed them to one, and
    /// a caller who is not a member would be handed them as asked, which is the
    /// household's whole view. So no such stream exists for anybody else.
    #[must_use]
    pub fn for_member(ctx: Arc<Ctx>, caller: &Caller) -> Option<Self> {
        let member = caller.member()?.to_owned();
        Some(Self {
            ctx,
            member,
            heard: Mutex::new(Heard::default()),
        })
    }

    /// The id of the member this stream is theirs.
    #[must_use]
    pub fn member(&self) -> &str {
        &self.member
    }
}

#[async_trait]
impl Gathers for Theirs {
    async fn gather(&self, joined: bool) -> Vec<Rendered> {
        self.gathered(joined).await
    }
}

impl Theirs {
    /// What is said this time round: every reading whose pace has come round, and of
    /// those only what changed since it was last said, unless the listener has just
    /// joined and holds nothing yet.
    async fn gathered(&self, joined: bool) -> Vec<Rendered> {
        let mut heard = self.heard.lock().await;
        let now = Instant::now();
        let mut asking = Vec::new();
        if joined || due(heard.rows, ROWS_EVERY, now) {
            heard.rows = Some(now);
            asking.extend([Reading::Household, Reading::Held]);
        }
        if joined || due(heard.playing, PLAYING_EVERY, now) {
            heard.playing = Some(now);
            asking.push(Reading::Playing);
        }
        let mut read = Vec::new();
        for reading in asking {
            read.extend(self.answered(reading).await);
        }
        let mut said = Vec::new();
        for rendered in read {
            let previous = heard
                .said
                .insert(rendered.kind(), rendered.said().to_owned());
            if joined || previous.as_deref() != Some(rendered.said()) {
                said.push(rendered);
            }
        }
        said
    }

    /// One reading, narrowed to the member, as the event it is said as. Nothing where
    /// the answer will not render.
    async fn answered(&self, reading: Reading) -> Option<Rendered> {
        let outcome = match dispatch(reading.theirs(&self.member), &self.ctx).await {
            Ok(outcome) => outcome,
            Err(_) => reading.unread(),
        };
        Rendered::of(Nature::State, &outcome.envelope())
    }
}

/// The three things a member's stream reads, and nothing else it could be asked to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reading {
    /// Their household row.
    Household,
    /// Their shelf.
    Held,
    /// What they are playing.
    Playing,
}

impl Reading {
    /// The command narrowed to `member`, by the same functions that narrow it when the
    /// member asks for it themselves.
    fn theirs(self, member: &str) -> Command {
        match self {
            Self::Household => their_household(member),
            Self::Held => their_shelf(member, A_SHELF),
            Self::Playing => their_playing(member),
        }
    }

    /// The reading where the stack could not be read, said as unread rather than empty.
    fn unread(self) -> Outcome {
        let findings = vec![UNREAD.to_owned()];
        match self {
            Self::Household => Outcome::Household(HouseholdReport {
                findings,
                ..HouseholdReport::default()
            }),
            Self::Held => Outcome::Held(HeldReport {
                findings,
                ..HeldReport::default()
            }),
            Self::Playing => Outcome::Playing(PlayingReport {
                findings,
                ..PlayingReport::default()
            }),
        }
    }
}

/// Whether a pace that last came round at `last` has come round again.
fn due(last: Option<Instant>, every: Duration, now: Instant) -> bool {
    last.is_none_or(|last| now.duration_since(last) >= every)
}

#[cfg(test)]
mod tests;
