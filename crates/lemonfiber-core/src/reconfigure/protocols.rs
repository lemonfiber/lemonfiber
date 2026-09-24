//! What adding a way of downloading opens, and what dropping one keeps.
//!
//! The commonest reconfiguration there is: somebody starts library-only and wants
//! Usenet, or starts on Usenet and wants torrents. Growing into a full stack one
//! decision at a time is a supported path rather than a repair, which means each
//! step opens exactly what the new protocol needs and nothing else — an operator
//! adding Usenet asked no question about a tunnel and must not be handed one.
//!
//! Dropping one is the same path walked backwards, and the sentence that matters
//! is the opposite one. What stops is obvious from what was switched off; what is
//! *kept* is not, and it is the whole of what somebody is weighing when they drop
//! the way they built their library with. So it is stated in full rather than left
//! to be inferred from silence.
//!
//! Both answers are read off what the stack and the walk already declare — the
//! profiles a protocol guards, and the steps setup puts for it. A second list here
//! would be a list that could disagree with the questions actually asked.

use std::path::Path;

use lemonfiber_manifest::Manifest;

use super::Opening;
use crate::config::{reads_as_on, Protocols, TORRENT_KEY, USENET_KEY};
use crate::prerequisites::prerequisites;
use crate::stack::closure::everything;
use crate::wizard::opened_by;

/// What the protocols become when `key` is set to `value`.
///
/// `None` where the setting is not one of the two that decide them, which is every
/// other setting a change can name.
#[must_use]
pub fn changed(now: Protocols, key: &str, value: &str) -> Option<Protocols> {
    let on = reads_as_on(value);
    match key {
        USENET_KEY => Some(Protocols { usenet: on, ..now }),
        TORRENT_KEY => Some(Protocols { torrent: on, ..now }),
        _ => None,
    }
}

/// What moving from `before` to `after` newly asks the operator for.
///
/// The accounts they have to go and obtain, then the settings the answers are kept
/// in — in that order, because an account has to exist before a credential for it
/// can be pasted anywhere. Empty for a reduction, which asks for nothing.
#[must_use]
pub fn opened(before: Protocols, after: Protocols) -> Vec<Opening> {
    let had = prerequisites(before).items;
    let mut opens: Vec<Opening> = prerequisites(after)
        .items
        .into_iter()
        .filter(|item| !had.iter().any(|held| held.id == item.id))
        .map(|item| Opening {
            what: item.label.to_owned(),
            because: item.why.to_owned(),
            setting: None,
        })
        .collect();
    opens.extend(opened_by(before, after).into_iter().flat_map(|step| {
        step.settings().iter().map(move |setting| Opening {
            what: (*setting).to_owned(),
            because: format!("{} is asked for once this is on", step.label()),
            setting: Some((*setting).to_owned()),
        })
    }));
    opens
}

/// The services `after` stops that `before` was running, by the name the stack
/// gives them.
///
/// The stack's own answer: a profile declares the protocol it cannot run without,
/// so what a reduction takes away is read from the manifest rather than from a list
/// here that a renamed profile would silently outlive. Empty where the reduction
/// leaves the stack with nothing to run at all, which the closure refuses to plan
/// rather than answer.
#[must_use]
pub fn stopped(manifest: &Manifest, before: Protocols, after: Protocols) -> Vec<String> {
    let Ok(was) = everything(manifest, before) else {
        return Vec::new();
    };
    let now = everything(manifest, after).map(|plan| plan.services);
    let now = now.unwrap_or_default();
    was.services
        .into_iter()
        .filter(|service| !now.contains(service))
        .collect()
}

/// What dropping a way of downloading leaves exactly as it is.
///
/// Stated in full and with the paths where they are known, because the fear this
/// answers is specific: that switching off torrents takes the library built with
/// them. It does not. The change writes one setting; nothing walks the operator's
/// disk, and nothing removes a service's record of what it already has.
///
/// Empty for a change that takes nothing away, which has nothing to reassure
/// anybody about.
#[must_use]
pub fn kept(before: Protocols, after: Protocols, root: Option<&Path>) -> Vec<String> {
    if !reduces(before, after) {
        return Vec::new();
    }
    let under = |what: &str, folder: &str| {
        root.map_or_else(
            || format!("everything already {what}"),
            |root| {
                format!(
                    "everything already {what}, in {}",
                    root.join(folder).display()
                )
            },
        )
    };
    vec![
        under("downloaded", "downloads"),
        under("imported into the library", "media"),
        "each service's own record of what it holds, so nothing is searched for a second time"
            .to_owned(),
    ]
}

/// Whether the change takes a way of downloading away.
#[must_use]
pub(crate) const fn reduces(before: Protocols, after: Protocols) -> bool {
    (before.usenet && !after.usenet) || (before.torrent && !after.torrent)
}

#[cfg(test)]
mod tests;
