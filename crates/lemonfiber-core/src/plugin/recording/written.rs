//! Writing a recording where its manifest names it: beneath the plugin's own source,
//! through no link, keeping the note already there.

use std::path::{Path, PathBuf};

use super::Took;
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

/// Write a recording of `of`, taken from `pinned`, at `fixture` beneath `root`, with the
/// note the recording already there holds.
pub(super) fn written(root: &Path, fixture: &str, pinned: &str, of: Of) -> Took {
    let at = match landing(root, fixture) {
        Ok(at) => at,
        Err(why) => return Took::Unrecorded { why },
    };
    let note = before(root, fixture)
        .map(|before| before.note)
        .unwrap_or_default();
    let recording = Recording {
        recorded_from: pinned.to_owned(),
        note,
        of,
    };
    let unnoted = recording.note.is_empty();
    let held = std::fs::read_to_string(&at).ok();
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
