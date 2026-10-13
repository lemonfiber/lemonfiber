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

/// Where and when the snapshot this build carries was taken, as the release-time script
/// wrote it.
const TAKEN: &str = include_str!("roots/google-status/taken.json");

/// The snapshot's entries, one file per first character of the serial.
const SHARDS: [&str; 16] = [
    include_str!("roots/google-status/0.json"),
    include_str!("roots/google-status/1.json"),
    include_str!("roots/google-status/2.json"),
    include_str!("roots/google-status/3.json"),
    include_str!("roots/google-status/4.json"),
    include_str!("roots/google-status/5.json"),
    include_str!("roots/google-status/6.json"),
    include_str!("roots/google-status/7.json"),
    include_str!("roots/google-status/8.json"),
    include_str!("roots/google-status/9.json"),
    include_str!("roots/google-status/a.json"),
    include_str!("roots/google-status/b.json"),
    include_str!("roots/google-status/c.json"),
    include_str!("roots/google-status/d.json"),
    include_str!("roots/google-status/e.json"),
    include_str!("roots/google-status/f.json"),
];

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

/// Where and when a snapshot was taken.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Taken {
    /// Where the list was read from.
    #[expect(
        dead_code,
        reason = "read so a snapshot naming no source does not read"
    )]
    source: String,
    /// When it was read, as every instant this product writes is.
    fetched: String,
}

/// One shard of a snapshot's entries.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Shard {
    /// Each certificate named, by its serial number in lower-case hex or in decimal.
    entries: BTreeMap<String, Entry>,
}

/// A snapshot of the list.
#[derive(Debug, Clone)]
pub(crate) struct Snapshot {
    /// When it was taken, as every instant this product writes is.
    fetched: String,
    /// Each certificate named, by its serial number in its normal form.
    entries: BTreeMap<String, Entry>,
}

/// The snapshot this build carries, or nothing where it does not read.
static CARRIED: LazyLock<Option<Snapshot>> = LazyLock::new(|| Snapshot::read(TAKEN, &SHARDS));

impl Snapshot {
    /// The snapshot `taken` and `shards` hold, each serial in its normal form, or nothing
    /// where any of them does not read, or the time it was taken is not an instant.
    pub(crate) fn read(taken: &str, shards: &[&str]) -> Option<Self> {
        let taken: Taken = serde_json::from_str(taken).ok()?;
        crate::instant::read(&taken.fetched)?;
        let mut entries = BTreeMap::new();
        for shard in shards {
            let shard: Shard = serde_json::from_str(shard).ok()?;
            entries.extend(
                shard
                    .entries
                    .into_iter()
                    .map(|(serial, entry)| (normal(&serial), entry)),
            );
        }
        Some(Self {
            fetched: taken.fetched,
            entries,
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
