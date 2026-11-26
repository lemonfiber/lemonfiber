//! The contract's examples, written by running every read, every action and every
//! route beside them against the core's fixture fakes, and keeping what each answered,
//! the release record an answer carries cut to a fixed few of each list.
//!
//! Every world has a stopped clock, randomness that cycles, and a directory of its own
//! that every answer names by one stand-in path, so what is written is the same on
//! every machine and on every run.

mod acted;
mod asked;
mod authoring;
mod beside;
mod caller;
mod coded;
mod companion;
mod devices;
mod disk;
mod house;
mod record;
mod standing;
mod surface;
mod surfacing;
mod terminal;
mod world;

use std::fs::File;
use std::path::Path;

use lemonfiber_api::contract::example::{Acted, Examples};
use serde_json::{Map, Value};

/// Every example a run against the fakes answered with.
#[must_use]
pub fn gathered() -> Examples {
    let _alone = std::fs::create_dir_all(world::ROOT)
        .and_then(|()| File::create(Path::new(world::ROOT).with_extension("lock")))
        .and_then(|held| held.lock().map(|()| held));
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .ok()
        .and_then(|runtime| runtime.block_on(gather()))
        .unwrap_or_default()
}

/// The reads first, so a kind a read answers is exemplified by that read, then the
/// routes beside them, then the actions and what their work came to.
async fn gather() -> Option<Examples> {
    let mut seen = Seen::default();
    asked::every(&mut seen).await?;
    beside::every(&mut seen).await?;
    devices::every(&mut seen).await?;
    coded::every(&mut seen).await?;
    acted::every(&mut seen).await?;
    authoring::every(&mut seen).await?;
    terminal::every(&mut seen).await?;
    standing::every(&mut seen).await?;
    caller::every(&mut seen).await?;
    surfacing::every(&mut seen).await?;
    Some(seen.examples)
}

/// What gathering has kept so far.
#[derive(Default)]
pub(crate) struct Seen {
    /// The first envelope of each kind, and each action as it was taken.
    examples: Examples,
}

impl Seen {
    /// Each of `envelopes` kept as its kind's example, where that kind has none yet.
    pub(crate) fn answered(&mut self, envelopes: impl IntoIterator<Item = Value>) {
        for mut envelope in envelopes {
            record::abridged(&mut envelope);
            let kind = envelope
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            self.examples.kinds.entry(kind).or_insert(envelope);
        }
    }

    /// `action` as it was sent with `arguments` and what it answered.
    pub(crate) fn acted(&mut self, action: &str, arguments: Map<String, Value>, answered: Value) {
        self.examples.actions.insert(
            action.to_owned(),
            Acted {
                arguments,
                answered,
            },
        );
    }
}
