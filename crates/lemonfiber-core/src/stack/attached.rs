//! Which networks each of the stack's services is on.
//!
//! Read from the compose files rather than the manifest, because the networks are where
//! the stack writes down who can reach whom: the request service reaches the curators
//! it hands requests to and the media server it signs in against only through the
//! request gate, over networks the gate shares with those services alone, and nothing
//! in the manifest says so.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

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
    for (_, text) in files {
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
