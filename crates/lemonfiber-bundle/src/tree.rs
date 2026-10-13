//! A plugin's tree and its digest.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::ErrorKind;
use std::path::Path;

use crate::copies::{bytes, linked, walked};
use crate::parts::declared;
use crate::{hex, Refused, DIGEST_PREFIX, MANIFEST};

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

/// The digest of the tree of the plugin whose repository is checked out at `root`, read
/// with `manifest` as its manifest's text: the manifest, each service's starting files,
/// every fixture the manifest names and its conformance.
///
/// # Errors
///
/// [`Refused`] where a file of the tree cannot be read, or where any path of it is or
/// passes through a symbolic link.
pub fn of(root: &Path, manifest: &str) -> Result<String, Refused> {
    let part = declared(manifest);
    let mut files = BTreeMap::new();
    let mut links = Vec::new();
    let directories = part
        .services
        .iter()
        .map(|service| format!("{CONFIG}/{}", service.id))
        .chain([CONFORMANCE.to_owned()]);
    for directory in directories {
        if let Some(link) = linked(root, &directory) {
            links.push(link);
            continue;
        }
        let at = root.join(&directory);
        if !at.is_dir() {
            continue;
        }
        let copy = walked(&at)?;
        links.extend(copy.links.iter().map(|path| format!("{directory}/{path}")));
        files.extend(
            copy.files
                .into_iter()
                .map(|(path, bytes)| (format!("{directory}/{path}"), bytes)),
        );
    }
    for fixture in &part.fixtures {
        if let Some(link) = linked(root, fixture) {
            links.push(link);
            continue;
        }
        match bytes(&root.join(fixture)) {
            Ok(read) => {
                files.insert(fixture.clone(), read);
            }
            Err(Refused::Unread { source, .. }) if source.kind() == ErrorKind::NotFound => {}
            Err(refused) => return Err(refused),
        }
    }
    if links.is_empty() {
        Ok(digest(manifest, &files))
    } else {
        links.sort();
        links.dedup();
        Err(Refused::Rules(
            links
                .into_iter()
                .map(|at| format!("{at} is a symbolic link, which nothing a tree holds may be"))
                .collect(),
        ))
    }
}

#[cfg(test)]
mod tests;
