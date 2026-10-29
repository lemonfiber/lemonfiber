//! The one action that reverses each kind of change.

use super::{Action, Change, Kind, Undo};

impl Action {
    /// Put `field` of the service's `resource` `id` back to `value`.
    fn reconfigure(resource: &str, id: &str, field: &str, value: Option<&str>) -> Self {
        Self::Reconfigure {
            resource: resource.to_owned(),
            id: id.to_owned(),
            field: field.to_owned(),
            value: value.map(str::to_owned),
        }
    }

    /// Start `service` where `running`, and stop it otherwise.
    fn run(service: &str, running: bool) -> Self {
        Self::Run {
            service: service.to_owned(),
            running,
        }
    }

    /// Leave `service`'s internal certificate as it is.
    fn unreplaced(service: &str) -> Self {
        Self::Unreplaced {
            service: service.to_owned(),
        }
    }

    /// Give `grant` to `plugin` again where `give`, and end it otherwise.
    fn regrant(plugin: &str, grant: &crate::plugin::Grant, give: bool) -> Self {
        Self::Regrant {
            plugin: plugin.to_owned(),
            grant: grant.clone(),
            give,
        }
    }
}

impl Change {
    /// The action that reverses this change.
    #[must_use]
    pub fn undo(&self) -> Undo {
        Undo {
            target: self.target.clone(),
            action: self.reversal(),
        }
    }

    /// What reversing this change does.
    fn reversal(&self) -> Action {
        match &self.kind {
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
            Kind::Renamed { from, to } => Action::RenameBack {
                from: from.clone(),
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
            } => Action::reconfigure(resource, id, field, previous.as_deref()),
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
                grant,
                given,
            } => Action::regrant(plugin, grant, !given),
            Kind::Overlaid { path, .. } => Action::Leave { path: path.clone() },
            Kind::Ran { service, running } => Action::run(service, !running),
            Kind::Certified { service, .. } => Action::unreplaced(service),
        }
    }
}
