//! A plugin's tree and its digest.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::{hex, DIGEST_PREFIX, MANIFEST};

/// Where a service's starting files sit in a plugin's tree.
pub(crate) const CONFIG: &str = "config";

/// Where a plugin's conformance recordings sit in its tree.
pub(crate) const CONFORMANCE: &str = "conformance";

/// The tree's digest, `sha256:<hex>`: the SHA-256 of one line per file, sorted by its
/// path's bytes, each the file's SHA-256 in lower-case hex, two spaces, its path and a
/// line feed.
#[must_use]
pub fn digest(manifest: &str, files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut lines: Vec<(&[u8], String)> = files
        .iter()
        .map(|(path, bytes)| (path.as_bytes(), hex(bytes)))
        .collect();
    lines.push((MANIFEST.as_bytes(), hex(manifest.as_bytes())));
    lines.sort_by(|left, right| left.0.cmp(right.0));
    let mut listing = String::new();
    for (path, sum) in &lines {
        let _ = writeln!(listing, "{sum}  {}", String::from_utf8_lossy(path));
    }
    format!("{DIGEST_PREFIX}{}", hex(listing.as_bytes()))
}

/// Whether `path` belongs to a tree whose manifest declares `services` and names
/// `fixtures`.
pub(crate) fn holds(path: &str, services: &[String], fixtures: &[String]) -> bool {
    path.strip_prefix(CONFORMANCE)
        .is_some_and(|rest| rest.starts_with('/'))
        || fixtures.iter().any(|fixture| fixture == path)
        || services
            .iter()
            .any(|id| path.starts_with(&format!("{CONFIG}/{id}/")))
}
