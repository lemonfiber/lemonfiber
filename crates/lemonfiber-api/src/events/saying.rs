//! What a long wait says, put on the stream everybody is already listening to.
//!
//! A browser that asked for a form to be started is answered with a name for the
//! work and nothing else, because the work outlives the request. Everything it
//! learns afterwards comes down this stream — so a wait that said nothing left a
//! dashboard showing the same figures for minutes, which is the browser's version
//! of a terminal that has gone quiet.
//!
//! Nothing is rendered here that the command line does not render the same way: the
//! words are the core's, and this only wraps them in the envelope every other event
//! arrives in.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::ports::Narrator;

use super::live::Live;
use super::wire::{Nature, Rendered};

/// A wait, speaking onto the one stream this run keeps.
pub struct Saying {
    /// What is said, and everyone hearing it.
    live: Arc<Live>,
}

impl Saying {
    /// Say onto this stream.
    #[must_use]
    pub const fn onto(live: Arc<Live>) -> Self {
        Self { live }
    }
}

#[async_trait]
impl Narrator for Saying {
    /// Say one line to every listener.
    ///
    /// Carried as state rather than as a record: only the newest line describes
    /// what the wait is waiting for now, so a client that was away is caught up
    /// with where the wait got to instead of being replayed every second of it.
    async fn say(&self, said: &str) {
        if let Some(rendered) = Rendered::of(Nature::State, &Envelope::new(kind::START, said)) {
            self.live.say(rendered).await;
        }
    }
}
