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
    /// The work this wait belongs to, by the name its accepting reply gave it.
    job: Option<String>,
}

impl Saying {
    /// Say onto this stream, for work no job names.
    #[must_use]
    pub const fn onto(live: Arc<Live>) -> Self {
        Self { live, job: None }
    }

    /// Say onto this stream, for the work this job names.
    ///
    /// Every line it says carries the name, so a client that asked for the work
    /// ties what the wait says to the request it made.
    #[must_use]
    pub fn for_job(live: Arc<Live>, job: &str) -> Self {
        Self {
            live,
            job: Some(job.to_owned()),
        }
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
        said_to(&self.live, self.job.as_deref(), said).await;
    }
}

/// One line, rendered and handed to the listeners — or dropped where it will not render.
///
/// The dropping is [`Live::say_if_rendered`]'s rather than a branch here, because a
/// payload that will not render is a case this cannot stage and that one is already
/// driven where it lives.
async fn said_to(live: &Live, job: Option<&str>, said: &str) {
    live.say_if_rendered(Rendered::of(
        Nature::State,
        &Envelope::new(kind::START, said).said_by(job),
    ))
    .await;
}
