use lemonfiber_manifest::container::DOMAIN;

use crate::config::env::EnvFile;
use crate::config::{
    DOMAIN_KEY, HOUSEHOLD_HOST_KEY, HOUSEHOLD_HOST_SETTING, VPN_PORT_FORWARDING_KEY,
    VPN_PROVIDER_KEY,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Legacy {
    pub(crate) key: &'static str,
    pub(crate) name: &'static str,
    pub(crate) secret: bool,
}

const fn setting(key: &'static str, name: &'static str) -> Legacy {
    Legacy {
        key,
        name,
        secret: false,
    }
}

pub(crate) const TABLE: [Legacy; 11] = [
    setting(VPN_PROVIDER_KEY, "gluetun:provider"),
    setting("VPN_COUNTRIES", "gluetun:countries"),
    setting(VPN_PORT_FORWARDING_KEY, "gluetun:port-forwarding"),
    setting("FLARESOLVERR_LOG_LEVEL", "flaresolverr:log-level"),
    setting("SEERR_LOG_LEVEL", "seerr:log-level"),
    setting("RECYCLARR_CRON", "recyclarr:cron"),
    setting("UMASK", "unpackerr:umask"),
    setting("HOMEPAGE_ALLOWED_HOSTS", "homepage:allowed-hosts"),
    setting(DOMAIN_KEY, DOMAIN),
    setting(HOUSEHOLD_HOST_KEY, HOUSEHOLD_HOST_SETTING),
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
