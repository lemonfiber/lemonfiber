//! The small JSON records kept beside the configuration.
//!
//! Five of them now — the conditions, the notification appetite, the answered
//! choices, what was last materialised, the adopted baseline — and every one had
//! written out the same three lines: read the file if there is one, parse it if it
//! parses, and fall back to the default. Four copies of a rule is four places for
//! it to drift, and the rule here is one worth stating once.
//!
//! **Reading is best effort and writing is not.** A record that cannot be read
//! leaves the default, which puts a settled question again or forgets how long a
//! fault has stood — tiresome, and always in the safe direction. A record that
//! cannot be *written* is a different thing: silence there would leave the
//! operator believing something was remembered that was not, so it is reported.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::config::store;

use super::Ctx;
use crate::error::{Diagnose, Problem};

/// What the record holds, or the default where there is none to read.
///
/// Nothing configured means nowhere to keep it, which is not a fault: a machine
/// with no configuration has no history to remember either.
#[must_use]
pub(crate) fn kept<T: Default + DeserializeOwned>(path: Option<&Path>) -> T {
    path.and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Read the record kept beside the environment file, or the default where there is none.
///
/// The name is the whole of what differs between these records, so it is the whole of what
/// a caller passes. This module's own doc counted five of them and said four copies of a
/// rule is four places to drift; the body was factored then and the wrapper was not, and by
/// the seventh record that wrapper was the duplication.
#[must_use]
pub(crate) fn beside<T: Default + DeserializeOwned>(ctx: &Ctx, name: &str) -> T {
    kept(super::targets::beside_env(ctx, name).as_deref())
}

/// Write one beside the environment file, best effort.
///
/// Best effort on the way out, unlike a record an operator decided: a history that could not
/// be written is one the next run starts afresh from, which is a worse picture rather than a
/// wrong claim, and never worth failing a command over.
pub(crate) fn keep_beside<T: Serialize>(ctx: &Ctx, name: &str, value: &T) {
    let _ = keep(super::targets::beside_env(ctx, name).as_deref(), value);
}

/// Write the record where the next run will read it.
///
/// # Errors
///
/// Where there is nowhere configured to keep it, or the file cannot be written.
pub(crate) fn keep<T: Serialize>(path: Option<&Path>, value: &T) -> Result<(), Box<Problem>> {
    let path = path.ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    // A value that will not serialise writes as an empty record rather than
    // refusing: the types here are plain data with derived implementations, so it
    // cannot happen, and inventing an error path for it would be a branch no test
    // could ever reach.
    store::write(path, &serde_json::to_string(value).unwrap_or_default())
        .map_err(|failure| Box::new(failure.problem()))
}

#[cfg(test)]
mod tests;
