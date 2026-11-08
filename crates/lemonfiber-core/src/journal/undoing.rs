//! The one action that reverses each kind of change.

use super::{Action, Change, Kind, Undo};

impl Action {
    /// Write `key` back to `value` where it still holds `wrote`.
    fn restore(key: &str, value: Option<&str>, wrote: &str) -> Self {
        Self::Restore {
            key: key.to_owned(),
            value: value.map(str::to_owned),
            wrote: wrote.to_owned(),
        }
    }

    /// Put the key now under `to` back under `from`.
    fn rename_back(from: &str, to: &str) -> Self {
        Self::RenameBack {
            from: from.to_owned(),
            to: to.to_owned(),
        }
    }

    /// Move what is now at `to` back to `from`.
    fn move_back(from: &str, to: &str) -> Self {
        Self::MoveBack {
            from: from.to_owned(),
            to: to.to_owned(),
        }
    }

    /// Remove the service's `resource` `id`.
    fn remove(resource: &str, id: &str) -> Self {
        Self::Remove {
            resource: resource.to_owned(),
            id: id.to_owned(),
        }
    }

    /// Ask a curator to grab the release fingerprinted `release` of its `kind` title
    /// `title` again.
    fn regrab(kind: crate::ports::media::Kind, title: i64, release: &str) -> Self {
        Self::Regrab {
            kind,
            title,
            release: release.to_owned(),
        }
    }

    /// Put `field` of the service's `resource` `id` back to `value`.
    fn reconfigure(resource: &str, id: &str, field: &str, value: Option<&str>) -> Self {
        Self::Reconfigure {
            resource: resource.to_owned(),
            id: id.to_owned(),
            field: field.to_owned(),
            value: value.map(str::to_owned),
        }
    }

    /// Take `owner`'s region out of `path`, recorded under `key`, where it still holds what
    /// was written, its checksum `written`.
    fn withdraw(path: &str, key: &str, owner: &str, written: u32) -> Self {
        Self::Withdraw {
            path: path.to_owned(),
            key: key.to_owned(),
            owner: owner.to_owned(),
            written,
        }
    }

    /// Write `previous` back into `path`, where it still holds what was written, its
    /// checksum `written`.
    fn rewind(path: &str, previous: &str, written: u32) -> Self {
        Self::Rewind {
            path: path.to_owned(),
            previous: previous.to_owned(),
            written,
        }
    }

    /// Write the value the store holds of `plugin`'s `secret` back into `path`, where it
    /// still holds what was written, its checksum `written`.
    fn reseed(path: &str, plugin: &str, secret: &str, written: u32) -> Self {
        Self::Reseed {
            path: path.to_owned(),
            plugin: plugin.to_owned(),
            secret: secret.to_owned(),
            written,
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

    /// Leave the guard that took `service`'s place on its plugin.
    fn unmoved(service: &str) -> Self {
        Self::Unmoved {
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
            Kind::Created { resource, id } => Action::remove(resource, id),
            Kind::Set {
                key,
                previous,
                current,
            } => Action::restore(key, previous.as_deref(), current),
            Kind::Made { path } => Action::Delete { path: path.clone() },
            Kind::Region {
                path,
                key,
                owner,
                written,
            } => Action::withdraw(path, key, owner, *written),
            Kind::Rewritten {
                path,
                previous,
                written,
            } => Action::rewind(path, previous, *written),
            Kind::Seeded {
                path,
                plugin,
                secret,
                written,
            } => Action::reseed(path, plugin, secret, *written),
            Kind::Copied { volume, to } => Action::KeepCopy {
                volume: volume.clone(),
                to: to.clone(),
            },
            Kind::Renamed { from, to } => Action::rename_back(from, to),
            Kind::Moved { from, to } => Action::move_back(from, to),
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
            Kind::PushEndpointRegistered { name } | Kind::PushEndpointRemoved { name } => {
                Action::PushEndpoint { name: name.clone() }
            }
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
            Kind::Restarted { service } => Action::run(service, true),
            Kind::Certified { service, .. } => Action::unreplaced(service),
            Kind::MovedInPlace { service, .. } => Action::unmoved(service),
            Kind::Unqueued {
                kind,
                title,
                release,
            } => Action::regrab(*kind, *title, release),
        }
    }
}
