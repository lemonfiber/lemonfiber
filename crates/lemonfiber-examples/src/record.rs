//! What an example keeps of the release record an answer carries.
//!
//! An answer naming the record carries every release it holds and every requirement the
//! running release cites, so it grows with each release. An example keeps a fixed few of
//! each list and the requirements its kept entries cite, so it stays one size release
//! after release and still shows every field.

use std::collections::BTreeSet;

use serde_json::Value;

/// How many releases, groups of the running release and entries in each group an
/// example keeps.
const KEPT: usize = 2;

/// `envelope` with the release record it carries cut to [`KEPT`] of each list, and to
/// the requirements its kept entries cite.
pub(crate) fn abridged(envelope: &mut Value) {
    let Some(notes) = envelope.pointer_mut("/data/changelog") else {
        return;
    };
    kept(notes.get_mut("releases"));
    let mut cited = BTreeSet::new();
    for group in kept(notes.pointer_mut("/running/groups")) {
        for entry in kept(group.get_mut("entries")) {
            let listed = entry.get("requirements").and_then(Value::as_array);
            cited.extend(
                listed
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned),
            );
        }
    }
    if let Some(requirements) = notes.get_mut("requirements").and_then(Value::as_object_mut) {
        requirements.retain(|id, _| cited.contains(id));
    }
}

/// The first [`KEPT`] of `list`, the rest dropped, or nothing where it is not a list.
fn kept(list: Option<&mut Value>) -> &mut [Value] {
    list.and_then(Value::as_array_mut).map_or(&mut [], |list| {
        list.truncate(KEPT);
        list.as_mut_slice()
    })
}

#[cfg(test)]
mod tests;
