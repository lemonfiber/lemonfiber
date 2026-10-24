//! The conformance recordings beside a manifest that no case asked: removed, or left
//! as they are and named where the run writes nothing.

use std::collections::BTreeSet;
use std::path::Path;

use super::{Retaking, Taken, Took};
use crate::plugin::conforming::RECORDINGS;

/// What names a recording no case asked.
const NAMED: &str = "no case";

/// Each conformance recording beneath `root` that none of `taken` names, removed where
/// `taking` writes and named either way.
pub(super) fn taken(root: &Path, taken: &[Taken], taking: &Retaking) -> Vec<Taken> {
    let asked: BTreeSet<&str> = taken
        .iter()
        .filter_map(|one| one.fixture.as_deref())
        .collect();
    let mut found = Vec::new();
    if std::fs::symlink_metadata(root.join(RECORDINGS)).is_ok_and(|kind| kind.is_dir()) {
        held(root, RECORDINGS, &mut found);
    }
    found.sort();
    found
        .into_iter()
        .filter(|fixture| !asked.contains(fixture.as_str()))
        .map(|fixture| Taken {
            named: NAMED.to_owned(),
            took: if taking.check {
                Took::Unasked
            } else {
                removed(root, &fixture)
            },
            fixture: Some(fixture),
        })
        .collect()
}

/// Each recording beneath `under`, relative to `root`, into `found`, through no link.
fn held(root: &Path, under: &str, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(root.join(under)) else {
        return;
    };
    for entry in entries.flatten() {
        let kind = entry.file_type().ok();
        let name = entry.file_name().to_string_lossy().into_owned();
        let at = format!("{under}/{name}");
        if kind.is_some_and(|kind| kind.is_dir()) {
            held(root, &at, found);
        } else if kind.is_some_and(|kind| kind.is_file())
            && Path::new(&name)
                .extension()
                .is_some_and(|ext| ext == "json")
        {
            found.push(at);
        }
    }
}

/// What removing the recording at `fixture` beneath `root`, found through no link, came to.
fn removed(root: &Path, fixture: &str) -> Took {
    let at = root.join(fixture);
    std::fs::remove_file(&at).map_or_else(
        |why| Took::Unrecorded {
            why: format!("{} could not be removed: {why}", at.display()),
        },
        |()| Took::Removed,
    )
}

#[cfg(test)]
mod tests;
