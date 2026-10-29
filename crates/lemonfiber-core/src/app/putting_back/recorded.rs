//! The record a reversal keeps of itself: each change it put back, written the other way
//! round.

use crate::journal::{Change, Undo};

use super::OPERATION;

/// The record a reversal keeps of itself, so it can be put back in turn.
///
/// One entry per change actually put back, with the values the other way round: what the
/// original change wrote is what this one found, and what it restored is what stands now.
/// Asking for *this* run then puts the original values back, which is what makes a
/// reversal answerable to the same command as everything else.
///
/// Two of the four kinds record nothing, and the reason is the journal's vocabulary
/// rather than an omission. A path that was made is gone once the reversal removes it,
/// and `Made` would say it had been created; what a service created is never put back at
/// all, so there is nothing to record. Neither has an inverse this record can spell, and
/// inventing one would put a line in the journal that replaying would not reproduce.
pub(super) fn recording(run: &[&Change], reversed: &[Undo], at: &str) -> Vec<Change> {
    run.iter()
        .filter(|change| reversed.iter().any(|undo| undo.target == change.target))
        .filter_map(|change| {
            inverted(&change.kind).map(|kind| Change {
                at: at.to_owned(),
                operation: OPERATION.to_owned(),
                target: change.target.clone(),
                kind,
            })
        })
        .collect()
}

/// One change with the values it moved between swapped, where that can be said at all.
fn inverted(kind: &crate::journal::Kind) -> Option<crate::journal::Kind> {
    use crate::journal::Kind;
    match kind {
        Kind::Set {
            key,
            previous,
            current,
        } => Some(Kind::Set {
            key: key.clone(),
            previous: Some(current.clone()),
            current: previous.clone().unwrap_or_default(),
        }),
        Kind::Configured {
            resource,
            id,
            field,
            previous,
            current,
        } => Some(Kind::Configured {
            resource: resource.clone(),
            id: id.clone(),
            field: field.clone(),
            previous: Some(current.clone()),
            current: previous.clone().unwrap_or_default(),
        }),
        Kind::KeyMinted { name, scope } | Kind::KeyCollected { name, scope } => {
            Some(Kind::KeyRevoked {
                name: name.clone(),
                scope: scope.clone(),
            })
        }
        Kind::DeviceKeyRegistered { name, .. } => {
            Some(Kind::DeviceKeyRevoked { name: name.clone() })
        }
        Kind::Setting {
            name,
            previous,
            current,
            ..
        } => Some(Kind::Setting {
            name: name.clone(),
            previous: current.clone(),
            current: previous.clone(),
            filed: None,
        }),
        Kind::Secret {
            name,
            from,
            previous,
            current,
            ..
        } => Some(Kind::Secret {
            name: name.clone(),
            from: from.clone(),
            previous: current.clone(),
            current: previous.clone(),
            filed: None,
        }),
        Kind::Moved { from, to } => Some(Kind::Moved {
            from: to.clone(),
            to: from.clone(),
        }),
        Kind::Renamed { from, to } => Some(Kind::Renamed {
            from: to.clone(),
            to: from.clone(),
        }),
        Kind::Granted {
            plugin,
            grant,
            given,
        } => Some(Kind::Granted {
            plugin: plugin.clone(),
            grant: grant.clone(),
            given: !given,
        }),
        Kind::Ran { service, running } => Some(Kind::Ran {
            service: service.clone(),
            running: !running,
        }),
        Kind::Created { .. }
        | Kind::Made { .. }
        | Kind::Copied { .. }
        | Kind::Region { .. }
        | Kind::Rewritten { .. }
        | Kind::Pinned { .. }
        | Kind::KeyRevoked { .. }
        | Kind::DeviceKeyRevoked { .. }
        | Kind::Overlaid { .. }
        | Kind::Restarted { .. }
        | Kind::Certified { .. } => None,
    }
}

#[cfg(test)]
mod tests;
