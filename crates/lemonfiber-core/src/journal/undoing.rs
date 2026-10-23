//! The one action that reverses each kind of change.

use super::{Action, Change, Kind, Undo};

impl Change {
    /// The action that reverses this change.
    #[must_use]
    pub fn undo(&self) -> Undo {
        let action = match &self.kind {
            Kind::Created { resource, id } => Action::Remove {
                resource: resource.clone(),
                id: id.clone(),
            },
            Kind::Set {
                key,
                previous,
                current,
            } => Action::Restore {
                key: key.clone(),
                value: previous.clone(),
                wrote: current.clone(),
            },
            Kind::Made { path } => Action::Delete { path: path.clone() },
            Kind::Region {
                path,
                key,
                owner,
                written,
            } => Action::Withdraw {
                path: path.clone(),
                key: key.clone(),
                owner: owner.clone(),
                written: *written,
            },
            Kind::Rewritten {
                path,
                previous,
                written,
            } => Action::Rewind {
                path: path.clone(),
                previous: previous.clone(),
                written: *written,
            },
            Kind::Copied { volume, to } => Action::KeepCopy {
                volume: volume.clone(),
                to: to.clone(),
            },
            Kind::Moved { from, to } => Action::MoveBack {
                from: from.clone(),
                to: to.clone(),
            },
            Kind::Pinned {
                previous, current, ..
            } => Action::Repin {
                previous: previous.clone(),
                current: current.clone(),
            },
            Kind::Configured {
                resource,
                id,
                field,
                previous,
                ..
            } => Action::Reconfigure {
                resource: resource.clone(),
                id: id.clone(),
                field: field.clone(),
                value: previous.clone(),
            },
            Kind::KeyMinted { name, .. } | Kind::KeyCollected { name, .. } => {
                Action::Revoke { name: name.clone() }
            }
            Kind::KeyRevoked { name, .. } => Action::Reinstate { name: name.clone() },
            Kind::DeviceKeyRegistered { name, .. } => Action::RevokeDevice { name: name.clone() },
            Kind::DeviceKeyRevoked { name } => Action::ReinstateDevice { name: name.clone() },
            Kind::Setting {
                name,
                previous,
                current,
                filed,
            } => Action::Resettle {
                name: name.clone(),
                value: previous.clone(),
                recorded: current.clone(),
                filed: filed.clone(),
            },
            Kind::Secret {
                name,
                previous,
                current,
                filed,
                ..
            } => Action::Rehold {
                name: name.clone(),
                earlier: previous.clone(),
                kept: current.clone(),
                filed: filed.clone(),
            },
            Kind::Granted {
                plugin,
                credential,
                of,
                serves,
                at,
                given,
            } => Action::Regrant {
                plugin: plugin.clone(),
                credential: credential.clone(),
                of: of.clone(),
                serves: serves.clone(),
                at: at.clone(),
                give: !given,
            },
            Kind::Overlaid { path, .. } => Action::Leave { path: path.clone() },
        };
        Undo {
            target: self.target.clone(),
            action,
        }
    }
}
