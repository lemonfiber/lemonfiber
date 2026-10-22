//! A plugin's tree and its digest.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::ErrorKind;
use std::path::{Component, Path};

use crate::copies::{found, linked, summed};
use crate::parts::declared;
use crate::{hex, Refused, DIGEST_PREFIX, MANIFEST};

/// Where a service's starting files sit in a plugin's tree.
pub(crate) const CONFIG: &str = "config";

/// Where a plugin's conformance recordings sit in its tree.
pub(crate) const CONFORMANCE: &str = "conformance";

/// Where an assembled part's claims keep their fixtures, beneath a folder of its id.
pub(crate) const RECORDINGS: &str = "recordings";

/// The tree's digest, `sha256:<hex>`: the SHA-256 of one line per file, sorted by its
/// path's bytes, each the file's SHA-256 in lower-case hex, two spaces, its path and a
/// line feed.
#[must_use]
pub fn digest(manifest: &str, files: &BTreeMap<String, Vec<u8>>) -> String {
    listed(
        manifest,
        files
            .iter()
            .map(|(path, bytes)| (path.clone(), hex(bytes)))
            .collect(),
    )
}

/// The tree's digest from each file's SHA-256 by its path, the manifest's beside them.
fn listed(manifest: &str, mut sums: BTreeMap<String, String>) -> String {
    sums.insert(MANIFEST.to_owned(), hex(manifest.as_bytes()));
    let mut listing = String::new();
    for (path, sum) in &sums {
        let _ = writeln!(listing, "{sum}  {path}");
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
/// every fixture the manifest names and its conformance. Each file is hashed in pieces,
/// never held whole.
///
/// # Errors
///
/// [`Refused`] where a file of the tree cannot be read, where any path of it is or
/// passes through a symbolic link, or where a path the manifest names is not one inside
/// the repository.
pub fn of(root: &Path, manifest: &str) -> Result<String, Refused> {
    let part = declared(manifest);
    let mut sums = BTreeMap::new();
    let mut refused = Vec::new();
    let directories = part
        .services
        .iter()
        .map(|service| format!("{CONFIG}/{}", service.id))
        .chain([CONFORMANCE.to_owned()]);
    for directory in directories {
        if !inside(&directory) {
            refused.push(format!("{directory} is not a path inside the repository"));
            continue;
        }
        if let Some(link) = linked(root, &directory) {
            refused.push(linking(&link));
            continue;
        }
        let at = root.join(&directory);
        if !at.is_dir() {
            continue;
        }
        let (paths, links) = found(&at)?;
        refused.extend(
            links
                .iter()
                .map(|path| linking(&format!("{directory}/{path}"))),
        );
        for path in paths {
            sums.insert(format!("{directory}/{path}"), summed(&at.join(&path))?);
        }
    }
    for fixture in &part.fixtures {
        if !inside(fixture) {
            refused.push(format!("{fixture} is not a path inside the repository"));
            continue;
        }
        if let Some(link) = linked(root, fixture) {
            refused.push(linking(&link));
            continue;
        }
        match summed(&root.join(fixture)) {
            Ok(sum) => {
                sums.insert(fixture.clone(), sum);
            }
            Err(Refused::Unread { source, .. }) if source.kind() == ErrorKind::NotFound => {}
            Err(unreadable) => return Err(unreadable),
        }
    }
    if refused.is_empty() {
        Ok(listed(manifest, sums))
    } else {
        refused.sort();
        refused.dedup();
        Err(Refused::Rules(refused))
    }
}

/// Whether `relative` names a path beneath the repository's root: plain names joined by
/// `/`, none of them empty, `.` or `..`, and no root or prefix.
fn inside(relative: &str) -> bool {
    relative
        .split('/')
        .all(|name| !name.is_empty() && name != "." && name != "..")
        && Path::new(relative)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// The refusal of a link at `path`.
fn linking(path: &str) -> String {
    format!("{path} is a symbolic link, which nothing a tree holds may be")
}

#[cfg(test)]
mod tests;
