//! Which networks each of the stack's services is on.
//!
//! Read from the compose files rather than the manifest, because the networks are where
//! the stack writes down who can reach whom: the request service reaches the curators
//! it hands requests to and the media server it signs in against only through the
//! request gate, over networks the gate shares with those services alone, and nothing
//! in the manifest says so.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::mounts::Networking;

/// The network Compose puts a service on where it names none.
pub const DEFAULT: &str = "default";

/// Every service the compose files declare, with the networks it is on.
///
/// The ones it names, in either syntax and in every file that declares it, since Compose
/// merges a service's networks across the files that make the project; the default
/// network where no file names one; and none at all where it takes another container's
/// network in place of its own, since such a service is on no network of its own to be
/// joined beside.
#[must_use]
pub(crate) fn attached(files: &[(PathBuf, String)]) -> BTreeMap<String, BTreeSet<String>> {
    let mut declared: BTreeMap<String, Declared> = BTreeMap::new();
    for (_, text) in files.iter().filter(|(path, _)| !brought(path)) {
        for Networking {
            service,
            borrowed,
            named,
        } in super::mounts::networking(text)
        {
            let one = declared.entry(service).or_default();
            one.borrowed |= borrowed;
            one.named.extend(named);
        }
    }
    declared
        .into_iter()
        .map(|(name, one)| {
            let on = match (one.borrowed, one.named.is_empty()) {
                (true, _) => BTreeSet::new(),
                (false, true) => BTreeSet::from([DEFAULT.to_owned()]),
                (false, false) => one.named,
            };
            (name, on)
        })
        .collect()
}

/// Whether the file is a plugin's own document rather than one of the stack's.
///
/// A plugin's entry is written from what lemonfiber worked out, so reading it back as
/// the stack's statement would let one plugin's networks become what every later
/// stand-in for a stack service of the same name is put on.
fn brought(path: &Path) -> bool {
    path.parent()
        .is_some_and(|directory| directory.ends_with(crate::plugin::OVERLAYS))
}

/// What the files declaring one service say of its networks, together.
#[derive(Default)]
struct Declared {
    /// Whether any takes another container's network.
    borrowed: bool,
    /// Every network any names.
    named: BTreeSet<String>,
}

#[cfg(test)]
mod tests;
