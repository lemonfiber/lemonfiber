//! The request gate's keys on a curator's adapter: each admitted on the operations a
//! curator answers the gate, and refused as disallowed on every other.

use std::io::Read as _;
use std::path::Path;

use axum::http::HeaderMap;
use lemonfiber_contract::adapter::{KEY_LIMIT, REQUEST_KEY_FILE};
use lemonfiber_contract::capabilities::library::{curate, REQUESTED};
use lemonfiber_contract::wire::Kind;
use lemonfiber_contract::Refusal;

use super::{offered, same_secret, Kit};

/// The most the request gate's keys may hold, in bytes: room for a key being rotated in
/// beside the one it replaces, and then some.
const KEYS_MOST: u64 = 4 * KEY_LIMIT as u64;

/// Who a request carries the key of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Caller {
    /// The core, under the plugin's key.
    Core,
    /// The request gate, under one of its keys.
    Gate,
    /// Neither.
    Nobody,
}

/// Who `headers` carry the key of.
pub(crate) fn caller(kit: &Kit, headers: &HeaderMap) -> Caller {
    let offered = offered(headers);
    if same_secret(&kit.key, offered) {
        return Caller::Core;
    }
    let gated = kit.gate.as_deref().is_some_and(|configuration| {
        keys_in(configuration)
            .iter()
            .any(|key| same_secret(key, offered))
    });
    if gated {
        Caller::Gate
    } else {
        Caller::Nobody
    }
}

/// Whether the request gate may ask `operation` of `capability` at `major`, as the path
/// names the major.
pub(crate) fn admits(capability: &str, major: &str, operation: &str) -> bool {
    capability == curate::CAPABILITY
        && major == format!("v{}", curate::MAJOR)
        && REQUESTED.contains(&operation)
}

/// The refusal for a request under the request gate's key of anything it may not ask.
pub(crate) fn disallowed(asked: &str) -> Refusal {
    Refusal::new(
        Kind::Disallowed,
        format!("the request gate's key does not open {asked}"),
    )
}

/// The request gate's keys, as the core last wrote them beside the adapter's
/// configuration: none where it wrote none, or what it wrote cannot be read whole.
fn keys_in(configuration: &Path) -> Vec<String> {
    let Ok(file) = std::fs::File::open(configuration.join(REQUEST_KEY_FILE)) else {
        return Vec::new();
    };
    let mut written = String::new();
    if file
        .take(KEYS_MOST + 1)
        .read_to_string(&mut written)
        .is_err()
        || written.len() as u64 > KEYS_MOST
    {
        return Vec::new();
    }
    written
        .lines()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests;
