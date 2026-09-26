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
//!
//! **And a record that cannot be read is never written over.** The default a
//! damaged record reads as is a stand-in for an answer, and writing the stand-in
//! back would replace whatever the file still held with it. So a write refuses
//! where a record is there and does not read as the type being written, and the
//! file is left for somebody to look at.

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
pub(crate) fn keep_beside<T: Serialize + DeserializeOwned>(ctx: &Ctx, name: &str, value: &T) {
    let _ = keep(super::targets::beside_env(ctx, name).as_deref(), value);
}

/// Write the record where the next run will read it.
///
/// # Errors
///
/// Where there is nowhere configured to keep it, or the file cannot be written.
pub(crate) fn keep<T: Serialize + DeserializeOwned>(
    path: Option<&Path>,
    value: &T,
) -> Result<(), Box<Problem>> {
    let path = path.ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    if let Some(reason) = unreplaceable::<T>(path) {
        return Err(Box::new(
            store::Failure::Unreadable {
                path: path.to_path_buf(),
                reason,
            }
            .problem(),
        ));
    }
    // A value that will not serialise writes as an empty record rather than
    // refusing: the types here are plain data with derived implementations, so it
    // cannot happen, and inventing an error path for it would be a branch no test
    // could ever reach.
    store::write(path, &serde_json::to_string(value).unwrap_or_default())
        .map_err(|failure| Box::new(failure.problem()))
}

/// Why the record at `path` may not be written over, where it may not.
///
/// Nothing where there is no file, or where the file reads as the record being
/// written: that is a record this build can read, and replacing it is the write
/// the caller asked for. Something where the file is there and cannot be read, or
/// reads as something else, because the value about to replace it was worked out
/// from the default a damaged record reads as.
fn unreplaceable<T: DeserializeOwned>(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => Some(error.to_string()),
        Ok(text) => serde_json::from_str::<T>(&text).err().map(|why| {
            format!(
                "it does not read as the record lemonfiber keeps there ({why}), so it is left as \
                 it is rather than written over"
            )
        }),
    }
}

#[cfg(test)]
mod tests;
