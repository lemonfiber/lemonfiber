//! What a walk has just done, put on the stream everybody is already listening to.
//!
//! A browser that asked for a walkthrough is answered with a name for the work and
//! nothing else, and the walk's whole value is what it says while it runs — an
//! operator watching their stack search, grab, download and import one thing is
//! the reason the command exists. Handed only the report at the end, they would
//! have learned what happened rather than watched it happen, which is a different
//! and much smaller thing.
//!
//! The step goes down whole rather than as a sentence. Every word in it is the
//! core's — what the step is called, and the evidence that makes it worth reading —
//! and rendering them into a line here would put a second copy of the walk's own
//! prose in this crate, beside the one the terminal already draws. Two copies of an
//! explanation drift, and the one that drifts is always the one fewer people read.
//!
//! Said through a channel because the two sides disagree about waiting: a walk
//! says its lines from ordinary code that cannot wait, and the stream is reached
//! by waiting. A channel is what makes that a handover rather than a spawn per
//! line — and a spawn per line would put the steps on the stream in whatever order
//! the runtime got to them, which for a narration is the one thing that must hold.

use std::sync::Arc;

use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::walkthrough::{Line, Narrator};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::live::Live;
use super::wire::{Nature, Rendered};

/// A walk, speaking onto the one stream this run keeps.
pub struct Stepping(UnboundedSender<Line>);

impl Stepping {
    /// A narrator for a walk, and the run that carries what it says to the stream.
    ///
    /// Two halves because they belong to different places: the narrator goes into
    /// the context a command runs against, and the carrying is a task the surface
    /// starts beside the gather. Handed back together so neither can be wired
    /// without the other — a narrator whose lines nothing carries would be a walk
    /// narrating into a channel nobody reads.
    #[must_use]
    pub fn onto(live: Arc<Live>) -> (Self, Carrying) {
        let (said, heard) = unbounded_channel();
        (Self(said), Carrying { live, heard })
    }
}

impl Narrator for Stepping {
    /// Hand one step over, now.
    ///
    /// Nothing comes back and nothing can go wrong that a walk could act on: a run
    /// whose surface has gone away is a run that goes on running, and a walk made
    /// to fail because nobody was listening would be a walk made worse by the
    /// reporting added to it.
    fn said(&self, line: &Line) {
        let _heard = self.0.send(line.clone());
    }
}

/// The steps a walk has said, on their way to the stream.
pub struct Carrying {
    /// What is said, and everyone hearing it.
    live: Arc<Live>,
    /// The steps still to be said, oldest first.
    heard: UnboundedReceiver<Line>,
}

impl Carrying {
    /// Say each step on the stream, in the order the walk said it.
    ///
    /// Ends when the last narrator is dropped, which is when the run ends: a walk
    /// that finished leaves the channel open for the next one, so this is started
    /// once with the surface rather than once per walk.
    pub async fn carrying(mut self) {
        while let Some(line) = self.heard.recv().await {
            // A record rather than state: a step is something that happened, and a
            // client that missed one has a hole in the walk rather than an
            // out-of-date figure. The newest step does not describe the ones before
            // it, which is exactly what a wait's own narration does describe.
            let said = Rendered::of(Nature::Record, &Envelope::new(kind::STEP, &line));
            self.live.say_if_rendered(said).await;
        }
    }
}
