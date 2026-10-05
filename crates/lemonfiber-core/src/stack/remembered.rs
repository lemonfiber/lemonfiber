//! The checked manifest, remembered while what it was read from has not changed.
//!
//! Checking a manifest parses it and validates it, and for the stack this build
//! carries it parses every compose file as well. The dashboard alone asks for it
//! on every refresh, and the household beside it asks again; the answer changes only
//! when the file does. So the last answer is kept with what it was read from — the
//! stack this binary carries, or the operator's file as it stood, by its size and
//! when it was last written — and handed back while that still holds.
//!
//! Only a manifest that checked is kept. A refusal is worked out afresh each time,
//! so a stack the operator is in the middle of fixing is read again on the next ask.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use lemonfiber_manifest::{Date, Manifest};

use super::{Failure, Source, MANIFEST};

/// What a checked manifest was read from, and on which day it was checked.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Read {
    /// Where the manifest came from.
    from: From,
    /// The day it was checked on, since one rule is about a date having passed.
    today: Date,
}

/// Where a manifest came from, precisely enough to tell when it has changed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum From {
    /// The stack this binary carries, by where it is held in memory.
    Embedded(usize),
    /// The operator's file, by where it is, when it was last written and its size.
    External(PathBuf, SystemTime, u64),
}

/// The last manifest that checked, and what it was read from.
static CHECKED: Mutex<Option<(Read, Manifest)>> = Mutex::new(None);

/// The manifest `check` gives for this source today, kept for as long as the source
/// is unchanged.
pub(super) fn remembered(
    source: Source,
    today: Date,
    check: impl FnOnce() -> Result<Manifest, Failure>,
) -> Result<Manifest, Failure> {
    let read = read(source, today);
    if let Some(kept) = read.as_ref().and_then(kept) {
        return Ok(kept);
    }
    let checked = check()?;
    if let (Some(read), Ok(mut kept)) = (read, CHECKED.lock()) {
        *kept = Some((read, checked.clone()));
    }
    Ok(checked)
}

/// The manifest kept for exactly this read, where there is one.
fn kept(read: &Read) -> Option<Manifest> {
    let kept = CHECKED.lock().ok()?;
    kept.as_ref()
        .filter(|(was, _)| was == read)
        .map(|(_, manifest)| manifest.clone())
}

/// What this source is read from today, or nothing where its file cannot be looked
/// at — which is then checked afresh, and its refusal says why.
fn read(source: Source, today: Date) -> Option<Read> {
    let from = match source {
        Source::Embedded(dir) => From::Embedded(std::ptr::from_ref(dir).addr()),
        Source::External(path) => {
            let manifest = path.join(MANIFEST);
            let meta = std::fs::metadata(&manifest).ok()?;
            From::External(manifest, meta.modified().ok()?, meta.len())
        }
    };
    Some(Read { from, today })
}

#[cfg(test)]
mod tests;
