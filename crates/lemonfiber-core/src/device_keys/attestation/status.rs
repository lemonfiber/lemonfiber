//! Google's attestation status list, as the snapshot this build carries holds it.
//!
//! A certificate the list names was issued under an attestation key Google has revoked or
//! suspended, so a chain carrying one cannot be told from a chain made outside a phone's
//! hardware. The snapshot is taken at release time and embedded; nothing is fetched.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;
use x509_parser::prelude::X509Certificate;

use crate::secret::render;

/// The snapshot this build carries, as the release-time script wrote it.
const SNAPSHOT: &str = include_str!("roots/google-status.json");

/// What the list says of a certificate it names.
///
/// Spelt as Google's list spells it, which is read here and never written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Status {
    /// Revoked for good.
    #[serde(rename = "REVOKED")]
    Revoked,
    /// Suspended until Google says otherwise.
    #[serde(rename = "SUSPENDED")]
    Suspended,
}

/// One certificate the list names.
#[derive(Debug, Clone, Deserialize)]
struct Entry {
    /// What the list says of it.
    status: Status,
}

/// A snapshot of the list.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Snapshot {
    /// When it was taken, as every instant this product writes is.
    fetched: String,
    /// Each certificate named, by its serial number in lower-case hex or in decimal.
    entries: BTreeMap<String, Entry>,
}

/// The snapshot this build carries, or nothing where it does not read.
static CARRIED: LazyLock<Option<Snapshot>> = LazyLock::new(|| Snapshot::read(SNAPSHOT));

impl Snapshot {
    /// The snapshot `written` holds, each serial in its normal form, or nothing where it
    /// does not read as one.
    pub(crate) fn read(written: &str) -> Option<Self> {
        let read: Self = serde_json::from_str(written).ok()?;
        Some(Self {
            entries: read
                .entries
                .into_iter()
                .map(|(serial, entry)| (normal(&serial), entry))
                .collect(),
            ..read
        })
    }

    /// Whether this names any of `certificates` as revoked or suspended.
    ///
    /// A serial is looked up both in hex and in decimal, the two forms the list writes
    /// serials in, each in its normal form.
    pub(crate) fn names_any(&self, certificates: &[X509Certificate<'_>]) -> bool {
        certificates.iter().any(|certificate| {
            let hex = render(certificate.raw_serial());
            let decimal = certificate.tbs_certificate.serial.to_string();
            [normal(&hex), normal(&decimal)]
                .iter()
                .filter_map(|written| self.entries.get(written))
                .any(|entry| matches!(entry.status, Status::Revoked | Status::Suspended))
        })
    }
}

/// A serial number as it is looked up: lower-case, its leading zeros dropped.
fn normal(serial: &str) -> String {
    serial.trim_start_matches('0').to_ascii_lowercase()
}

/// The snapshot this build carries, or nothing where it does not read.
pub(crate) fn carried() -> Option<&'static Snapshot> {
    CARRIED.as_ref()
}

/// When the snapshot this build carries was taken, or nothing where it does not read.
#[must_use]
pub fn taken() -> Option<String> {
    carried().map(|snapshot| snapshot.fetched.clone())
}

#[cfg(test)]
mod tests;
