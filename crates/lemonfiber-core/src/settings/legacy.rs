use std::path::Path;

use lemonfiber_manifest::container::DOMAIN;

use crate::app::Ctx;
use crate::config::env::EnvFile;
use crate::config::paths::SECRETS;
use crate::config::store::Failure;
use crate::config::{
    CAPTURED_SUFFIX, CREDENTIAL_SUFFIXES, DOMAIN_KEY, HOUSEHOLD_HOST_KEY, HOUSEHOLD_HOST_SETTING,
    INDEXER_APIKEY_KEY, PENDING_SUFFIX, PROVIDER_PASS_KEY, VPN_PORT_FORWARDING_KEY,
    VPN_PROVIDER_KEY,
};
use crate::instant::Instant;
use crate::journal::{Change, Kind};
use crate::ports::random::Random;

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

macro_rules! secret {
    ($key:expr, $name:expr) => {
        Legacy {
            key: $key,
            name: $name,
            secret: true,
        }
    };
}

pub(crate) const TABLE: [Legacy; 13] = [
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
    secret!(crate::credential::VPN_PRIVATE_KEY, "gluetun:wireguard-key"),
    secret!(INDEXER_APIKEY_KEY, INDEXER_APIKEY_KEY),
    secret!(PROVIDER_PASS_KEY, PROVIDER_PASS_KEY),
];

pub(crate) fn named(name: &str) -> Option<&'static Legacy> {
    TABLE.iter().find(|row| row.name == name)
}

pub(crate) fn keyed(key: &str) -> Option<&'static Legacy> {
    TABLE.iter().find(|row| row.key == key)
}

/// The name the secret store holds the credential the environment file kept under `key`
/// as, where `key` names one: a secret of the table under its name, and a credential the
/// stack records for a service, its replacement pending, or a value a plugin's recipe
/// captured that an earlier release kept there, under the same name.
pub(crate) fn held_as(key: &str) -> Option<String> {
    if let Some(replaced) = key.strip_suffix(PENDING_SUFFIX) {
        return held_as(replaced).map(|name| format!("{name}{PENDING_SUFFIX}"));
    }
    if let Some(row) = keyed(key).filter(|row| row.secret) {
        return Some(row.name.to_owned());
    }
    CREDENTIAL_SUFFIXES
        .iter()
        .chain(std::iter::once(&CAPTURED_SUFFIX))
        .any(|ending| key.ends_with(ending))
        .then(|| key.to_owned())
}

/// The environment file at `env` without the line an earlier release wrote for the
/// credential `key`, once the secret store holds it or it is forgotten, so the file is
/// never a second home for it.
///
/// # Errors
///
/// Where the file cannot be read, or cannot be written without the line.
pub(crate) fn unfiled(env: &Path, key: &str) -> Result<(), Failure> {
    if crate::config::store::read(env)?.written(key).is_none() {
        return Ok(());
    }
    crate::config::store::unset(env, key)
}

/// Where the credential the environment file kept under `key` came from: the operator for
/// a secret of the table, a plugin's recipe for a value one captured, and lemonfiber for
/// one it minted for a service.
pub(crate) fn source_of(key: &str) -> super::Source {
    if keyed(key).is_some() {
        super::Source::Operator
    } else if key.ends_with(CAPTURED_SUFFIX) {
        super::Source::Recipe
    } else {
        super::Source::Minted
    }
}

pub(crate) fn setting_key(name: &str) -> Option<&'static str> {
    named(name).filter(|row| !row.secret).map(|row| row.key)
}

/// How a value entered under the environment file's name of a secret is journalled.
pub(crate) struct Entering<'a> {
    /// The operation it is journalled under.
    pub(crate) operation: &'a str,
    /// The time it is journalled at.
    pub(crate) stamp: Instant,
    /// The time the store records it entered at.
    pub(crate) at: u64,
    /// Where the secret store's key is drawn from, where it has none yet.
    pub(crate) random: &'a dyn Random,
}

/// Keep `value`, entered under the environment file's name `key`, in the secret store,
/// where `key` names a credential, under the name the store holds it as, journalled
/// through `journalled`
/// by the fingerprints of what it held before and holds now. A setting is entered with
/// `lemonfiber settings set`, which is the one writer of its name.
pub(crate) fn entered(
    env: &Path,
    (key, value): (&str, &str),
    entering: &Entering<'_>,
    journalled: &dyn Fn(&[Change]) -> Result<(), Failure>,
) -> Option<Result<(), Failure>> {
    let name = held_as(key)?;
    Some(
        super::secrets::kept(
            &env.with_file_name(SECRETS),
            &[super::Keeping {
                name: &name,
                value,
                from: super::Source::Operator,
                at: entering.at,
            }],
            entering.random,
            &|replaced| {
                let changes: Vec<Change> = replaced
                    .iter()
                    .map(|one| Change {
                        at: entering.stamp,
                        operation: entering.operation.to_owned(),
                        target: one.name.clone(),
                        kind: Kind::Secret {
                            name: one.name.clone(),
                            from: super::Source::Operator.as_str().to_owned(),
                            previous: one.previous.clone(),
                            current: one.current.clone(),
                            filed: None,
                        },
                    })
                    .collect();
                journalled(&changes)
            },
        )
        .and_then(|()| unfiled(env, key)),
    )
}

/// `file` without a line for any name a setting or a secret the core holds was read
/// into: lemonfiber's own lines, which hold neither.
#[must_use]
pub fn unstored(mut file: EnvFile) -> EnvFile {
    for row in &TABLE {
        file.remove(row.key);
    }
    file
}

/// `file`, the environment file, with each name a setting or a secret the core holds was
/// read into answering only what the stores this run reads hold for it: what every reader
/// of those names is handed, and nothing the file itself still says.
#[must_use]
pub fn overlaid(ctx: &Ctx, file: EnvFile) -> EnvFile {
    answered(&super::Stores::of(ctx), file)
}

/// `file` with each name a setting or a secret the core holds was read into answering
/// only what `stores` hold for it.
pub(crate) fn answered(stores: &super::Stores, mut file: EnvFile) -> EnvFile {
    for row in &TABLE {
        let held = if row.secret {
            stores.secrets.value(row.name)
        } else {
            stores.settings.value(row.name)
        };
        match held {
            Some(value) => file.set(row.key, value),
            None => file.remove(row.key),
        }
    }
    file
}
