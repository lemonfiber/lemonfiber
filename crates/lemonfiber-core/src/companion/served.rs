//! How the web surface was last served, written down by the run that served it.
//!
//! Pairing material names the address a phone should reach, and the port is the one
//! part of that address nothing but a serving run knows. So every run writes down the
//! port it took and whether it took it encrypted and on the network, and pairing reads
//! that back — from the terminal while nothing is being served as well as from the
//! surface itself — rather than either of them guessing.

use std::path::Path;

use serde::Deserialize;

/// Where it is written, beside the certificate.
const FILE: &str = "served.json";

/// How the web surface was last served.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Served {
    /// The port it listened on.
    pub port: u16,
    /// Whether it presented the certificate kept beside this, rather than plain text.
    pub encrypted: bool,
    /// Whether it was offered to the network, rather than to this machine alone.
    pub network: bool,
}

/// Write down how the surface is being served.
///
/// # Errors
///
/// What stopped it being written, in words.
pub fn record(directory: &Path, served: Served) -> Result<(), String> {
    // Written out rather than serialised: three scalars cannot fail to be written, and a
    // serialiser's failure branch here would be one nothing could reach.
    let text = format!(
        "{{\n  \"port\": {},\n  \"encrypted\": {},\n  \"network\": {}\n}}\n",
        served.port, served.encrypted, served.network
    );
    crate::config::store::write(&directory.join(FILE), &text).map_err(|why| why.to_string())
}

/// How the surface was last served, or nothing where no run wrote it down or what was
/// written is not this record.
#[must_use]
pub fn last(directory: &Path) -> Option<Served> {
    let text = std::fs::read_to_string(directory.join(FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests;
