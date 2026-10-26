//! Writing a recording where its manifest names it: beneath the plugin's own source,
//! through no link, keeping the note already there; or, where the run writes nothing,
//! holding the recording there to what was taken.

use std::path::{Path, PathBuf};

use super::alike::alike;
use super::{Retaking, Took};
use crate::plugin::recorded::{self, Of, Recording};

/// Where `fixture` lands beneath `root`, refused where it leads outside the source or
/// a link stands on the way to it or at its name.
pub(super) fn landing(root: &Path, fixture: &str) -> Result<PathBuf, String> {
    let inside = crate::within::beneath(fixture)
        .ok_or_else(|| format!("{fixture} leads outside the plugin's own source"))?;
    let at = root.join(inside);
    let mut here = root.to_path_buf();
    for step in at.strip_prefix(root).unwrap_or(&at).components() {
        here.push(step);
        if std::fs::symlink_metadata(&here).is_ok_and(|kind| kind.file_type().is_symlink()) {
            return Err(format!(
                "{} is a link, and a recording is written through none",
                here.display()
            ));
        }
    }
    Ok(at)
}

/// The recording already at `fixture`, where one this build reads is there.
pub(super) fn before(root: &Path, fixture: &str) -> Option<Recording> {
    recorded::read(root, fixture).ok()
}

/// How the sentence a recording's note ends with where it was taken with stand-ins begins.
pub(super) const STOOD_IN: &str = "Taken with the stand-ins the manifest declares for ";

/// How that sentence ends.
const STOOD_IN_END: &str = ", never with an operator's value.";

/// The note written for a recording, without the sentence naming its stand-ins.
fn own(note: &str) -> &str {
    note.split_once(STOOD_IN)
        .map_or(note, |(own, _)| own)
        .trim_end()
}

/// The note a recording carries: `own`, and where `stood_in` names any variable, the
/// sentence naming each.
fn noted(own: &str, stood_in: &[String]) -> String {
    if stood_in.is_empty() {
        return own.to_owned();
    }
    let said = format!("{STOOD_IN}{}{STOOD_IN_END}", stood_in.join(", "));
    if own.is_empty() {
        said
    } else {
        format!("{own} {said}")
    }
}

/// Write a recording of `of`, taken from `pinned` with stand-ins for each variable
/// `stood_in` names, at `fixture` beneath `root`, with the note the recording already there
/// holds, or hold the one there to it where `taking` writes nothing.
pub(super) fn written(
    root: &Path,
    fixture: &str,
    pinned: &str,
    of: Of,
    taking: &Retaking,
    stood_in: &[String],
) -> Took {
    let at = match landing(root, fixture) {
        Ok(at) => at,
        Err(why) => return Took::Unrecorded { why },
    };
    let note = before(root, fixture)
        .map(|before| before.note)
        .unwrap_or_default();
    let unnoted = own(&note).is_empty();
    let recording = Recording {
        recorded_from: pinned.to_owned(),
        note: noted(own(&note), stood_in),
        of,
    };
    let held = std::fs::read_to_string(&at).ok();
    if taking.check {
        return checked(fixture, held.as_deref(), &recording, unnoted);
    }
    let made = crate::plugin::rendered(&recording)
        .ok_or(std::io::ErrorKind::InvalidData)
        .map_err(std::io::Error::from)
        .and_then(|text| {
            if held.as_deref() == Some(text.as_str()) {
                return Ok(false);
            }
            at.parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&at, text))
                .map(|()| true)
        });
    match made {
        Ok(_) if unnoted => Took::Unnoted,
        Ok(true) => Took::Written,
        Ok(false) => Took::Reproduced,
        Err(why) => Took::Unrecorded {
            why: format!("{} could not be written: {why}", at.display()),
        },
    }
}

/// What holding the recording `held` at `fixture` to `taken` comes to.
fn checked(fixture: &str, held: Option<&str>, taken: &Recording, unnoted: bool) -> Took {
    let Some(held) = held else {
        return Took::Unreproduced {
            why: format!("{fixture} is not there"),
        };
    };
    let read = |text: &str| serde_json::from_str::<serde_json::Value>(text).ok();
    let same = read(held)
        .zip(crate::plugin::rendered(taken).as_deref().and_then(read))
        .is_some_and(|(held, taken)| alike(&held, &taken));
    match (same, unnoted) {
        (false, _) => Took::Unreproduced {
            why: format!("{fixture} is not what was taken again"),
        },
        (true, true) => Took::Unnoted,
        (true, false) => Took::Reproduced,
    }
}
