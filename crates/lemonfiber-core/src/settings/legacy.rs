use std::path::Path;

use lemonfiber_manifest::container::DOMAIN;

use crate::config::env::EnvFile;
use crate::config::paths::{SECRETS, SETTINGS};
use crate::config::store::Failure;
use crate::config::{
    DOMAIN_KEY, HOUSEHOLD_HOST_KEY, HOUSEHOLD_HOST_SETTING, VPN_PORT_FORWARDING_KEY,
    VPN_PROVIDER_KEY,
};
use crate::credential::fingerprint;
use crate::journal::{Change, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Legacy {
    pub(crate) key: &'static str,
    pub(crate) name: &'static str,
    pub(crate) secret: bool,
}

macro_rules! setting {
    ($key:expr, $name:expr) => {
        Legacy {
            key: $key,
            name: $name,
            secret: false,
        }
    };
}

pub(crate) const TABLE: [Legacy; 11] = [
    setting!(VPN_PROVIDER_KEY, "gluetun:provider"),
    setting!("VPN_COUNTRIES", "gluetun:countries"),
    setting!(VPN_PORT_FORWARDING_KEY, "gluetun:port-forwarding"),
    setting!("FLARESOLVERR_LOG_LEVEL", "flaresolverr:log-level"),
    setting!("SEERR_LOG_LEVEL", "seerr:log-level"),
    setting!("RECYCLARR_CRON", "recyclarr:cron"),
    setting!("UMASK", "unpackerr:umask"),
    setting!("HOMEPAGE_ALLOWED_HOSTS", "homepage:allowed-hosts"),
    setting!(DOMAIN_KEY, DOMAIN),
    setting!(HOUSEHOLD_HOST_KEY, HOUSEHOLD_HOST_SETTING),
    Legacy {
        key: crate::credential::VPN_PRIVATE_KEY,
        name: "gluetun:wireguard-key",
        secret: true,
    },
];

pub(crate) fn named(name: &str) -> Option<&'static Legacy> {
    TABLE.iter().find(|row| row.name == name)
}

pub(crate) fn keyed(key: &str) -> Option<&'static Legacy> {
    TABLE.iter().find(|row| row.key == key)
}

pub(crate) fn setting_key(name: &str) -> Option<&'static str> {
    named(name).filter(|row| !row.secret).map(|row| row.key)
}

pub(crate) fn value<'a>(env: &'a EnvFile, name: &str) -> Option<&'a str> {
    setting_key(name)
        .and_then(|key| env.get(key))
        .filter(|value| !value.is_empty())
}

/// The journal entry for entering `value` under the environment file's name `key`, where
/// a secret holds what that name was read into: what it held before and holds now, by
/// their fingerprints alone. A setting is entered with `lemonfiber settings set`, which
/// is the one writer of its name.
pub(crate) fn entering(
    env: &Path,
    key: &str,
    value: &str,
    operation: &str,
    stamp: &str,
) -> Option<Change> {
    let row = keyed(key).filter(|row| row.secret)?;
    let kind = Kind::Secret {
        name: row.name.to_owned(),
        from: super::Source::Operator.as_str().to_owned(),
        previous: super::Secrets::at(&env.with_file_name(SECRETS))
            .ok()
            .and_then(|held| held.fingerprint(row.name)),
        current: Some(fingerprint(value)),
        filed: None,
    };
    Some(Change {
        at: stamp.to_owned(),
        operation: operation.to_owned(),
        target: row.name.to_owned(),
        kind,
    })
}

/// Keep `value`, entered under the environment file's name `key`, in the secrets store,
/// where a secret holds what that name was read into.
pub(crate) fn entered(env: &Path, key: &str, value: &str, at: u64) -> Option<Result<(), Failure>> {
    let row = keyed(key).filter(|row| row.secret)?;
    Some(super::secrets::kept(
        &env.with_file_name(SECRETS),
        &[super::Keeping {
            name: row.name,
            value,
            from: super::Source::Operator,
            at,
        }],
        &|_| Ok(()),
    ))
}

/// `file`, the environment file at `env`, with each name a setting the core holds was
/// read into answering what the settings store holds for it: what every reader of those
/// names is handed, once the stores hold the stack's values.
#[must_use]
pub fn overlaid(env: &Path, mut file: EnvFile) -> EnvFile {
    let Ok(held) = super::Settings::at(&env.with_file_name(SETTINGS)) else {
        return file;
    };
    for row in TABLE.iter().filter(|row| !row.secret) {
        if let Some(value) = held.value(row.name) {
            file.set(row.key, value);
        }
    }
    file
}

/// `file` as [`overlaid`] answers it, with each name a secret the core holds was read
/// into answering that secret too: the whole of what the operator set under those names,
/// for a listing that withholds every credential it shows.
pub(crate) fn listed(env: &Path, file: EnvFile) -> EnvFile {
    let mut file = overlaid(env, file);
    let Ok(kept) = super::Secrets::at(&env.with_file_name(SECRETS)) else {
        return file;
    };
    for row in TABLE.iter().filter(|row| row.secret) {
        if let Some(value) = kept.value(row.name) {
            file.set(row.key, value);
        }
    }
    file
}
