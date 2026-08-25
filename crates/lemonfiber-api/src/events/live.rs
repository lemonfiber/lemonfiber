//! One gather, and everyone listening to it.
//!
//! Concurrent surfaces must agree, and two gathers are two chances to disagree.
//! So there is one here, whatever is listening: a source is gathered from on a
//! tick, what it produced is said once, and every open stream hears the same
//! words. A second listener costs another subscriber, never another gather.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_core::ports::Clock;
use tokio::sync::{broadcast, Mutex, Notify};

use super::backlog::Backlog;
use super::wire::{Event, Rendered, BEAT, BEAT_SAID};

/// How often the source is gathered from when nobody has asked sooner.
///
/// The terminal dashboard's own tick, because it is the same gather: a figure
/// that is a second old on one surface must not be a minute old on another.
pub const TICK: Duration = Duration::from_secs(1);

/// How far behind a listener may fall before it is cut loose.
///
/// A listener that cannot keep up is ended rather than waited for, so one slow
/// client cannot hold up the others. It comes back saying where it got to, and
/// the backlog answers that properly.
const CARRIED: usize = 64;

/// Where the stream's events come from.
///
/// A port rather than a call, so what is gathered can be chosen — the live stack
/// in a run, something a test wrote in a test — without there being two gathers.
#[async_trait]
pub trait Gathers: Send + Sync {
    /// Gather once.
    ///
    /// `None` where there is nothing to say, which leaves the stream silent
    /// rather than saying something it did not gather.
    async fn gather(&self) -> Option<Rendered>;
}

/// What every listener hears, and what a returning one is caught up with.
pub struct Live {
    /// What is said, to whoever is listening when it is said.
    said: broadcast::Sender<Event>,
    /// What has been said, for whoever was not.
    backlog: Mutex<Backlog>,
    /// A listener asking for a gather now rather than at the next tick.
    wanted: Notify,
}

impl Live {
    /// A stream opening now, which has said nothing yet.
    #[must_use]
    pub fn opening(clock: &dyn Clock) -> Self {
        let (said, _) = broadcast::channel(CARRIED);
        Self {
            said,
            backlog: Mutex::new(Backlog::opening(clock)),
            wanted: Notify::new(),
        }
    }

    /// Say one thing to everyone listening, and to whoever comes next.
    ///
    /// Recorded and sent under the one lock, so a listener that subscribes
    /// between the two is neither told twice nor left with a hole.
    pub async fn say(&self, said: Rendered) -> Event {
        let mut backlog = self.backlog.lock().await;
        let event = backlog.say(said);
        // Nobody listening is not a failure: the stream exists whether or not a
        // browser is open, and what was said is in the backlog either way.
        let _sent = self.said.send(event.clone());
        event
    }

    /// Say one thing, where there was one to say.
    ///
    /// Rendering answers with an absence for a payload that will not serialise, and
    /// what to do about it is one decision rather than one per narrator: an
    /// envelope that cannot be rendered is not sent, rather than sent as something
    /// else. Taken here so a caller hands over what rendering gave it and nothing
    /// more.
    pub async fn say_if_rendered(&self, said: Option<Rendered>) {
        if let Some(said) = said {
            self.say(said).await;
        }
    }

    /// Listen, having last seen `seen`.
    pub async fn listening(&self, seen: Option<&str>) -> Listening {
        let backlog = self.backlog.lock().await;
        let said = self.said.subscribe();
        Listening {
            missed: backlog.since(seen).into(),
            said,
        }
    }

    /// Ask for a gather now rather than at the next tick.
    ///
    /// What a client holds from before it reconnected is stale until a gather
    /// made since replaces it, so the sooner one is made the shorter that is.
    pub fn nudge(&self) {
        self.wanted.notify_one();
    }

    /// One gather, said to everyone listening.
    pub async fn refresh(&self, source: &dyn Gathers) {
        if let Some(said) = source.gather().await {
            self.say(said).await;
        }
    }

    /// Gather on the tick, and whenever a listener asks, until the caller stops.
    pub async fn gathering(self: Arc<Self>, source: Arc<dyn Gathers>) {
        loop {
            self.refresh(source.as_ref()).await;
            tokio::select! {
                () = tokio::time::sleep(TICK) => {}
                () = self.wanted.notified() => {}
            }
        }
    }
}

/// One client's place in the stream.
pub struct Listening {
    /// The record it missed, oldest first, still to be handed over.
    missed: VecDeque<Event>,
    /// What is said from here on.
    said: broadcast::Receiver<Event>,
}

impl Listening {
    /// The next thing this client hears, or nothing where the stream has ended.
    ///
    /// A silence is broken on the beat rather than left to look like a
    /// connection that died, and the wait starts again from whatever was last
    /// said — so a busy stream beats only when it has gone quiet.
    pub async fn next(&mut self) -> Option<String> {
        if let Some(event) = self.missed.pop_front() {
            return Some(event.framed());
        }
        tokio::select! {
            heard = self.said.recv() => match heard {
                Ok(event) => Some(event.framed()),
                // Either the stream has ended, or this client fell so far behind
                // that what it missed is no longer here to hand over. Both end
                // the response: it comes back saying where it got to, which the
                // backlog can answer without guessing.
                Err(_) => None,
            },
            () = tokio::time::sleep(BEAT) => Some(BEAT_SAID.to_owned()),
        }
    }
}
