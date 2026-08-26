//! Reading a recorded value back out of the operator's own file.
//!
//! What each setting is *called* is declared next door, beside the list a writer
//! is held to. This is the other half: what a written value comes to, which is a
//! question with its own answers — a blank is a mistake and never an intent, and a
//! hand-added quote is not part of the value.
//!
//! Apart because they outgrew one file together, and they are two questions: the
//! one above is what may be written, and this is what it means once it is.

use std::path::PathBuf;

use super::env;
use super::reaching::offline;
use super::Indexer;
use super::{
    DATA_ROOT_KEY, DEFAULT_IP_ECHO, FRONT_DOOR_KEY, HOUSEHOLD_HOST_KEY, INDEXER_APIKEY_KEY,
    INDEXER_URL_KEY, IP_ECHO_KEY, PGID_KEY, PROVIDER_HOST_KEY, PUID_KEY, SECOND_IP_ECHO,
    VPN_PORT_FORWARDING_KEY, VPN_PROVIDER_KEY,
};

/// A recorded value with the whitespace and surrounding quotes a person might
/// add stripped, its case left alone — so a hand-edited `"https://IP.example"`
/// keeps its path but loses the quotes that would otherwise reach the reader.
fn unquoted(value: &str) -> String {
    value
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_owned()
}

/// The same, folded to lower case, so `"on"` and ` on ` both read as `on`.
fn bare(value: &str) -> String {
    unquoted(value).to_ascii_lowercase()
}

/// Whether a recorded setting reads as switched on.
///
/// Generous about spelling because this is a file people edit by hand, and a
/// setting that silently means "no" because it says `yes` rather than `on`
/// would be indistinguishable from lemonfiber ignoring it.
#[must_use]
pub fn reads_as_on(value: &str) -> bool {
    matches!(bare(value).as_str(), "on" | "true" | "yes" | "1")
}

/// Whether a recorded setting reads as switched off.
///
/// The counterpart to [`reads_as_on`] for a setting that also accepts a value:
/// an explicit off switches the feature off, where absence or an affirmative
/// would leave it on.
#[must_use]
pub fn reads_as_off(value: &str) -> bool {
    matches!(bare(value).as_str(), "off" | "false" | "no" | "0" | "")
}

/// The IP-echo service to verify egress against, or `None` where the operator
/// has switched leak detection off.
///
/// Absent or affirmative leaves the default in place; an explicit off disables
/// it, at the stated cost of losing leak detection; any other value replaces the
/// default with the operator's own endpoint — so the one third-party dependency
/// the check has is replaceable as well as disableable.
///
/// The blanket switch is read here rather than beside the other four, because this
/// setting names a source as well as answering yes or no: a source named under
/// `offline` is a source nothing may ask.
#[must_use]
pub fn ip_echo_from_env(file: &env::EnvFile) -> Vec<String> {
    let defaults = || vec![DEFAULT_IP_ECHO.to_owned(), SECOND_IP_ECHO.to_owned()];
    if offline(file) {
        return Vec::new();
    }
    match file.get(IP_ECHO_KEY) {
        None => defaults(),
        Some(value) if reads_as_on(value) => defaults(),
        Some(value) if reads_as_off(value) => Vec::new(),
        // Several, comma-separated, so an operator who wants their own sources can
        // still have more than one — the point of asking two is lost if naming one
        // silently drops back to trusting a single stranger.
        Some(value) => unquoted(value)
            .split(',')
            .map(str::trim)
            .filter(|source| !source.is_empty())
            .map(str::to_owned)
            .collect(),
    }
}

/// The Usenet provider's hostname, where one is recorded.
///
/// Blank is absent rather than a provider named the empty string, the way every
/// half-written setting here is read.
#[must_use]
pub fn provider_host_from_env(file: &env::EnvFile) -> Option<String> {
    file.get(PROVIDER_HOST_KEY)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Where the operator chose to keep downloads and media, if they have chosen.
///
/// Absent until setup writes it, and an empty value is read as absent rather
/// than as the current directory — a data root of nothing is a mistake, never an
/// intent, and probing the working directory in its place would test the wrong
/// filesystem.
#[must_use]
pub fn data_root_from_env(file: &env::EnvFile) -> Option<PathBuf> {
    file.get(DATA_ROOT_KEY)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The address the operator recorded for the household's links, if they have.
///
/// An empty value is read as absent rather than as an address of nothing, the same
/// reading the data root gets and for the same reason: a blank is a mistake, never
/// an intent.
#[must_use]
pub fn household_host_from_env(file: &env::EnvFile) -> Option<String> {
    file.get(HOUSEHOLD_HOST_KEY)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// The service the operator named as the front door, if they have named one.
///
/// An empty value is read as absent rather than as a service named nothing, the
/// same reading the household address gets and for the same reason: a blank is a
/// mistake, never an intent. Whether the name is one this stack may send a
/// household to is decided where the door is, not here — a setting that vetted
/// itself would be a second opinion about the household tier.
#[must_use]
pub fn front_door_from_env(file: &env::EnvFile) -> Option<String> {
    file.get(FRONT_DOOR_KEY)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// The indexer the operator configured, where both its URL and key are present.
///
/// Only a complete pair is an indexer to re-prove; a half-written one — a URL
/// with no key — is treated as none rather than a credential to test.
#[must_use]
pub fn indexer_from_env(file: &env::EnvFile) -> Option<Indexer> {
    let present = |key| {
        file.get(key)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    Some(Indexer {
        url: present(INDEXER_URL_KEY)?,
        key: present(INDEXER_APIKEY_KEY)?,
    })
}

/// The user and group the service containers run as, where both are configured.
///
/// Both or neither: deciding whether that user can write a directory needs the
/// pair, and one half without the other cannot answer the question, so a
/// half-configured file is treated as unconfigured rather than guessed at.
#[must_use]
pub fn service_user_from_env(file: &env::EnvFile) -> Option<(u32, u32)> {
    let uid = file.get(PUID_KEY)?.trim().parse().ok()?;
    let gid = file.get(PGID_KEY)?.trim().parse().ok()?;
    Some((uid, gid))
}

/// What the operator recorded about VPN port forwarding: whether they asked for
/// it, and which provider is meant to grant it.
///
/// Port forwarding is a per-provider capability — only some VPNs offer it — so
/// the provider name travels with the switch. The name is consulted not to decide
/// whether the tunnel works, but to name a provider's known trap when the switch
/// is on and no port arrives.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PortForward {
    /// Whether `VPN_PORT_FORWARDING` reads as switched on.
    pub enabled: bool,
    /// The provider name, lower-cased, where one is recorded.
    ///
    /// Carried even when the switch is off, but only read when it is on.
    pub provider: Option<String>,
}

/// What the operator recorded about port forwarding.
///
/// Absent or off leaves it disabled, in which case there is no forwarded port to
/// verify and the check does not apply. A blank provider is read as absent rather
/// than as a provider named the empty string.
#[must_use]
pub fn port_forward_from_env(file: &env::EnvFile) -> PortForward {
    PortForward {
        enabled: file.get(VPN_PORT_FORWARDING_KEY).is_some_and(reads_as_on),
        // `bare`, not a plain trim: the switch is de-quoted the same way, and a
        // provider left quoted would otherwise miss its known trap — the very
        // guidance the check exists to give — and read as an unknown provider.
        provider: file
            .get(VPN_PROVIDER_KEY)
            .map(bare)
            .filter(|value| !value.is_empty()),
    }
}
