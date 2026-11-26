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
    if let Some(releases) = notes.get_mut("releases").and_then(Value::as_array_mut) {
        releases.truncate(KEPT);
    }
    let mut cited = BTreeSet::new();
    if let Some(groups) = notes
        .pointer_mut("/running/groups")
        .and_then(Value::as_array_mut)
    {
        groups.truncate(KEPT);
        for entries in groups
            .iter_mut()
            .filter_map(|group| group.get_mut("entries").and_then(Value::as_array_mut))
        {
            entries.truncate(KEPT);
            cited.extend(
                entries
                    .iter()
                    .filter_map(|entry| entry.get("requirements").and_then(Value::as_array))
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
