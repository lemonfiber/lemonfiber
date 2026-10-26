//! Taking every recording a manifest names again, from its pin, and writing it or holding
//! the one there to it.
//!
//! A run proof's or a run probe's by running it, bounded as every proof's run is. Every
//! request's from one fresh container of its service, after the first run the core
//! performs for it, asked with the credential that first run left where its probe is
//! asked as the operator, and scrubbed of that credential and the container's identity.
//! A service is recorded whole or not at all, and the note already at each path is kept.

mod addressed;
mod alike;
mod asked;
mod first;
mod fronted;
mod ran;
mod scrubbed;
mod served;
mod unasked;
mod written;

pub(super) use fronted::handed_over;
pub(super) use served::{healthy, settled_as};

use std::path::Path;
use std::sync::Arc;

use lemonfiber_plugin::Manifest;
use serde::Serialize;

use super::claimed::{sourced, Unreadable};
use super::installed::{Installed, Placed};
use crate::doctor::BUNDLED_CHECKS;
use crate::ports::http::Http;
use crate::ports::program::Program;
use crate::ports::random::Random;
use crate::ports::serving::Serving;

/// How a recording run keeps what it takes, and what it takes it from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Retaking {
    /// Whether each recording is held to what was taken, and nothing written or removed,
    /// rather than each written and each no case asked removed.
    pub check: bool,
    /// An image the engine holds, each adapter started from in place of its pin.
    pub adapter: Option<String>,
}

impl Retaking {
    /// The image each adapter is started from in place of its pin, where one is named.
    fn adapter(&self) -> Option<&str> {
        self.adapter.as_deref()
    }
}

/// What a recording run reaches the outside through, and how it keeps what it takes.
#[derive(Clone, Copy)]
pub struct Recorder<'a> {
    /// Runs an image's own program once, for a run proof.
    pub program: &'a dyn Program,
    /// Serves a service's image, for its requests.
    pub serving: &'a dyn Serving,
    /// A fresh transport, holding no session, for each service's asking.
    pub transport: &'a (dyn Fn() -> Arc<dyn Http> + Send + Sync),
    /// Where a credential a first run mints comes from.
    pub random: &'a dyn Random,
    /// Whether what is taken is written or held to what is there.
    pub taking: &'a Retaking,
}

/// What taking one recording came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum Took {
    /// The recording was written.
    Written,
    /// The recording was written with an empty note, which its author has to write.
    Unnoted,
    /// The recording already held what was taken, byte for byte.
    Reproduced,
    /// The recording holds another result for the same image and arguments, so it is
    /// refused and left as it was.
    Unreproduced {
        /// What differs.
        why: String,
    },
    /// The run was still going at its limit, so there is nothing to record.
    Overran {
        /// The limit, in seconds.
        seconds: u32,
    },
    /// Nothing was taken, or there is nowhere to write what was.
    Unrecorded {
        /// Why.
        why: String,
    },
    /// The recording answers no case that was asked, so it was removed.
    Removed,
    /// The recording answers no case that was asked, and is left as it was because the
    /// run writes nothing.
    Unasked,
}

/// One recording a manifest names, and what taking it came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Taken {
    /// What names it: a proof, a claim's probe or a contributed check.
    pub named: String,
    /// The recording it names.
    pub fixture: Option<String>,
    /// What taking it came to.
    pub took: Took,
}

/// A service none of whose recordings was written, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Failed {
    /// The service.
    pub service: String,
    /// The step it failed at.
    pub step: String,
    /// Why.
    pub why: String,
    /// The last lines its container wrote, scrubbed and with nothing a terminal obeys.
    pub log: Vec<String>,
}

/// Every recording a plugin's manifest names, taken again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recorded {
    /// The plugin's id.
    pub id: String,
    /// Everything this build refuses about the manifest; nothing is taken where any is.
    pub refusals: Vec<lemonfiber_plugin::Violation>,
    /// Each recording, in the order the manifest names them.
    pub recordings: Vec<Taken>,
    /// Each service whose recordings were not taken.
    pub failed: Vec<Failed>,
    /// Whether every recording was written or reproduced, each with a note, and each no
    /// case asked removed.
    pub holds: bool,
}

/// Take every recording the manifest of the plugin whose source is at `path` names.
///
/// # Errors
///
/// [`Unreadable`] where there is no manifest at the path, or this build cannot read it.
pub async fn record(path: &Path, recorder: Recorder<'_>) -> Result<Recorded, Unreadable> {
    let (root, manifest) = sourced(path)?;
    let refusals = lemonfiber_plugin::refusals(&manifest, BUNDLED_CHECKS);
    let mut recordings = Vec::new();
    let mut failed = Vec::new();
    if refusals.is_empty() {
        for running in ran::runs(&manifest) {
            recordings.push(ran::taken(&manifest, running, &root, recorder).await);
        }
        let (asked, unfailed) = asked::taken(&manifest, &root, recorder).await;
        recordings.extend(asked);
        failed = unfailed;
        if failed.is_empty() {
            let strays = unasked::taken(&root, &recordings, recorder.taking);
            recordings.extend(strays);
        }
    }
    let holds = refusals.is_empty()
        && failed.is_empty()
        && recordings
            .iter()
            .all(|one| matches!(one.took, Took::Written | Took::Reproduced | Took::Removed));
    Ok(Recorded {
        id: manifest.plugin.id.clone(),
        refusals,
        recordings,
        failed,
        holds,
    })
}

/// The service named `named` asks of, as installing the manifest would place it.
fn placed(manifest: &Manifest, named: Option<&str>) -> Option<Placed> {
    manifest.asks(named).and_then(|service| {
        Installed::of(manifest)
            .services
            .into_iter()
            .find(|one| one.service == service.id)
    })
}

#[cfg(test)]
mod tests;
