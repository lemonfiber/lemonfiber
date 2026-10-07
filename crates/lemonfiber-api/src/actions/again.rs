//! An action sent again under the key it was first sent with.
//!
//! A client that heard nothing back cannot tell a request that never arrived from an
//! answer that never reached it, and the safe thing for it to do is send the same
//! action again. Without a way to recognise the second send, the stack would restart
//! twice or pause twice. So a request may carry an `Idempotency-Key`, and a second
//! request from the same caller under the same key is answered with what the first
//! was answered with: the same name for the same work, or the same outcome. The
//! action itself runs once.
//!
//! A key names one attempt, so it names one action and its arguments. The same key
//! with something else is refused rather than answered with an outcome that belongs
//! to a different request, and rather than run under a name it does not own.
//!
//! What is remembered is held in memory and nowhere else, per caller, for as long and
//! up to as many as the operator's settings say, read again on every send so a change
//! to either holds from the next one. A key serves the retry inside one attempt, and
//! an attempt does not outlive the run any more than its work does.

use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::SystemTime;

use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::{store, Resending};
use lemonfiber_core::model::{kind, Envelope};
use tokio::sync::{Mutex, SetOnce};
use tokio::task::JoinHandle;

use super::Arguments;
use crate::admission::Caller;
use crate::jobs::{accepted, Job};
use crate::read::enveloped;
use crate::refusal::Refusal;

/// The header a key travels in.
pub const HEADER: &str = "idempotency-key";

/// The most characters a key may have.
pub const LONGEST: usize = 255;

/// A key one attempt at an action was sent under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key(String);

impl Key {
    /// The key a request carried, nothing where it carried none, or the refusal for
    /// a key this surface does not read as one.
    ///
    /// Given twice is refused rather than read as either. Which of the two names the
    /// attempt is not something this can know, and guessing would recognise a second
    /// send by the wrong one.
    ///
    /// # Errors
    ///
    /// [`Refusal::NotAnIdempotencyKey`] where the header is given more than once or
    /// carries anything [`Key::read`] refuses.
    pub fn carried(headers: &HeaderMap) -> Result<Option<Self>, Refusal> {
        let mut given = headers.get_all(HEADER).iter();
        let Some(value) = given.next() else {
            return Ok(None);
        };
        match (given.next(), Self::read(value.as_bytes())) {
            (None, Some(key)) => Ok(Some(key)),
            _ => Err(Refusal::NotAnIdempotencyKey),
        }
    }

    /// A key from what a header carried: one to [`LONGEST`] visible characters.
    ///
    /// Visible and nothing else, so a key is the same string to every client that
    /// handles it. A space or a character outside ASCII is one a proxy or a library
    /// may rewrite, and a key rewritten between two sends is two keys.
    #[must_use]
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let fits = (1..=LONGEST).contains(&bytes.len()) && bytes.iter().all(u8::is_ascii_graphic);
        fits.then(|| Self(bytes.iter().copied().map(char::from).collect()))
    }
}

/// What an attempt asked for, as far as telling it apart from another is concerned.
///
/// A digest rather than the arguments themselves, so what is remembered is the same
/// size whatever an action was given. The digest is compared only within one
/// caller's own attempts under one key, so a collision is one a caller could only
/// arrange against itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asked(u64);

impl Asked {
    /// The action as it was named, and the arguments as they were read.
    #[must_use]
    pub fn of(action: &str, given: &Arguments) -> Self {
        let mut digest = DefaultHasher::new();
        action.hash(&mut digest);
        format!("{given:?}").hash(&mut digest);
        Self(digest.finish())
    }
}

/// What an attempt was answered with, kept so a second send is answered the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Carried out, at this status, with this envelope.
    Now(StatusCode, Option<String>),
    /// Handed to the runtime under this name, for the action named.
    Later(Job, String),
}

impl Answer {
    /// What an attempt whose work ended before it had an answer is answered with.
    fn unanswered() -> Self {
        let why = Refusal::Unanswered;
        let body = Envelope::new(kind::ERROR, why.problem(why.said())).to_json();
        Self::Now(why.status(), body)
    }

    /// The reply this answer is, as the first send was given it.
    #[must_use]
    pub fn reply(&self) -> Response {
        match self {
            Self::Now(status, body) => enveloped(*status, body.clone()),
            Self::Later(job, action) => accepted(job, action),
        }
    }
}

/// Where an attempt's answer arrives, once there is one.
pub type Slot = Arc<SetOnce<Answer>>;

/// What a request under a key turned out to be.
#[derive(Debug)]
pub enum Claim {
    /// The first send. Its answer is to be put in the slot.
    First(Slot),
    /// The same attempt sent again. Its answer arrives in the slot the first send fills.
    Again(Slot),
    /// The key already names an attempt that asked for something else.
    Otherwise,
}

/// One attempt remembered.
struct Held {
    /// Who sent it.
    who: Caller,
    /// The key it was sent under.
    key: Key,
    /// What it asked for.
    asked: Asked,
    /// When it was first sent.
    at: SystemTime,
    /// What it was answered with, once it has been.
    answer: Slot,
}

impl Held {
    /// Whether this is still being carried out.
    ///
    /// An attempt in flight is never let go, by age or to make room. Letting it go
    /// would let a second send run the action beside the first.
    fn in_flight(&self) -> bool {
        self.answer.get().is_none()
    }

    /// Whether this has been remembered for as long as an attempt is.
    ///
    /// A clock that has gone back reads as no time passed, so an attempt is never let
    /// go early.
    fn lapsed(&self, now: SystemTime, bounds: Resending) -> bool {
        now.duration_since(self.at)
            .is_ok_and(|age| age >= bounds.within)
    }
}

/// The attempts this run has answered, by who sent them and the key they carried.
#[derive(Default)]
pub struct Answered(Mutex<VecDeque<Held>>);

impl Answered {
    /// What a request under `key` is: the first send, a second, or a key reused.
    ///
    /// Recorded as it is claimed, under one lock, so two sends arriving together are
    /// one first send and one second. The second waits for the answer the first puts
    /// in the slot rather than running the action again.
    pub async fn claim(
        &self,
        who: &Caller,
        key: Key,
        asked: Asked,
        (now, bounds): (SystemTime, Resending),
    ) -> Claim {
        let mut held = self.0.lock().await;
        held.retain(|one| one.in_flight() || !one.lapsed(now, bounds));
        if let Some(one) = held.iter().find(|one| one.who == *who && one.key == key) {
            return if one.asked == asked {
                Claim::Again(Arc::clone(&one.answer))
            } else {
                Claim::Otherwise
            };
        }
        making_room(&mut held, who, bounds.at_most);
        let answer = Slot::default();
        held.push_back(Held {
            who: who.clone(),
            key,
            asked,
            at: now,
            answer: Arc::clone(&answer),
        });
        Claim::First(answer)
    }

    /// Put what `work` came to in `slot`, the first send's.
    ///
    /// Work that ended without coming to anything — stopped, or fallen over — is
    /// answered as that, so a second send waiting on the slot is never left waiting
    /// for good, and the attempt is forgotten: it reached no answer to give again.
    pub async fn settle(&self, slot: &Slot, work: JoinHandle<Answer>) {
        let finished = work.await.ok();
        if finished.is_none() {
            self.0
                .lock()
                .await
                .retain(|one| !Arc::ptr_eq(&one.answer, slot));
        }
        let _ = slot.set(finished.unwrap_or_else(Answer::unanswered));
    }
}

/// How long and how many, as the operator's settings say now.
///
/// Read from the file at each send rather than once when serving began, so a change
/// to either setting holds from the next send. A machine with no settings file, or
/// one that will not read, is held to the defaults.
#[must_use]
pub fn bounds(ctx: &Ctx) -> Resending {
    ctx.settings
        .env_file
        .as_deref()
        .and_then(|path| store::read(path).ok())
        .map_or_else(Resending::default, |file| Resending::from_env(&file))
}

/// Let go of `who`'s oldest answered attempts until there is room for one more
/// beside `at_most`.
///
/// Only `who`'s own, so one caller sending many keys cannot push out another
/// caller's attempt and let its second send run twice.
fn making_room(held: &mut VecDeque<Held>, who: &Caller, at_most: usize) {
    let theirs = held.iter().filter(|one| one.who == *who).count();
    let mut over = theirs.saturating_add(1).saturating_sub(at_most);
    held.retain(|one| {
        let going = over > 0 && one.who == *who && !one.in_flight();
        over = over.saturating_sub(usize::from(going));
        !going
    });
}

#[cfg(test)]
mod tests;
