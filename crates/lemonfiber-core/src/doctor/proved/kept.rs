//! The verdicts the doctor keeps for run proofs across restarts, each against everything
//! the run depends on.

use std::path::Path;

use lemonfiber_manifest::container::Posture;
use serde::{Deserialize, Serialize};

use crate::plugin::program::Operator;
use crate::plugin::{user_of, Placed, RunProof, Verdict};

/// Everything a run depends on, and so everything a kept verdict is kept against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Key {
    /// The image it runs, by digest.
    pub(crate) image: String,
    /// The arguments it hands the image's own program.
    pub(crate) args: Vec<String>,
    /// The posture it runs under.
    pub(crate) posture: Posture,
    /// Who it runs as: the service's user mode and the operator's ids.
    pub(crate) user: String,
    /// The digest of the starting files it runs with.
    pub(crate) starting: String,
    /// The lemonfiber release that ran it.
    pub(crate) release: String,
}

impl Key {
    /// What a run of `proof` for `placed` depends on now.
    pub(crate) fn of(
        placed: &Placed,
        proof: &RunProof,
        operator: &Operator,
        starting: String,
    ) -> Self {
        let mode = user_of(placed);
        Self {
            image: crate::plugin::program::pinned(placed),
            args: proof.run.args.clone(),
            posture: placed.fields.posture,
            user: format!("{mode:?} {}:{}", operator.ids.0, operator.ids.1),
            starting,
            release: crate::stack::RUNNING.to_owned(),
        }
    }
}

/// One verdict the doctor keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Kept {
    /// The plugin whose proof it is.
    pub(crate) plugin: String,
    /// The proof.
    pub(crate) proof: String,
    /// Everything the run depended on.
    pub(crate) key: Key,
    /// What it came to.
    pub(crate) verdict: Verdict,
    /// When it was reached, in whole seconds since the epoch.
    pub(crate) reached: String,
}

/// Every verdict kept at `at`, or none where nothing is or what is there cannot be read,
/// which costs a run and never a wrong verdict.
pub(crate) fn read(at: Option<&Path>) -> Vec<Kept> {
    at.and_then(|at| std::fs::read_to_string(at).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Keep `one` at `at` in place of any verdict in `held` for the same proof.
pub(crate) fn keep(at: Option<&Path>, held: Vec<Kept>, one: Kept) {
    let Some(at) = at else {
        return;
    };
    let mut kept: Vec<Kept> = held
        .into_iter()
        .filter(|earlier| !(earlier.plugin == one.plugin && earlier.proof == one.proof))
        .collect();
    kept.push(one);
    if let Ok(text) = serde_json::to_string(&kept) {
        let _ = crate::config::store::write(at, &text);
    }
}

#[cfg(test)]
mod tests;
