//! The single-mount rule: one mount beneath the data root, per service.
//!
//! Hardlinking is what makes an import cost nothing — the \*arr links the
//! downloaded file into the library instead of copying it, so the file exists in
//! both places, once on disk, and the torrent goes on seeding from it. A link
//! only works within one filesystem, and inside a container a bind mount *is* a
//! filesystem boundary. Two mounts beneath the data root put the download and the
//! library on opposite sides of one, and every import silently becomes a copy:
//! twice the disk, minutes instead of milliseconds, and seeding stops when the
//! original is cleaned up.
//!
//! It is invisible from the host. The hardlink probe runs on the host filesystem,
//! where the data root is one volume and links work perfectly — the breakage
//! exists only inside the container's view, which is why this is a rule about the
//! compose files rather than something a probe could ever catch.
//!
//! Read across the whole stack rather than file by file, because Compose's
//! `extends` carries volumes: a service that declares one mount and extends
//! something declaring another ends up with two, and a check reading one file at
//! a time would call that stack clean.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_yaml_ng::Value;

/// The variable the compose files name the data root by.
const DATA_ROOT: &str = "DATA_ROOT";

/// One service that would see more than one mount beneath the data root.
///
/// Public because what is done about it is not settled here. The stack lemonfiber
/// ships is refused over one of these; a stack directory the operator pointed at is
/// reported instead, by the storage check that states what it costs them. Both need
/// to be able to hold one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crowded {
    /// The service as the compose file names it.
    pub service: String,
    /// The mounts it would get, in the order they were declared.
    pub mounts: Vec<String>,
}

impl std::fmt::Display for Crowded {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "service {} mounts {} paths beneath the data root ({}); a bind mount is a \
             filesystem boundary inside the container, so anything imported across one is \
             copied rather than hardlinked — mount the data root once instead",
            self.service,
            self.mounts.len(),
            self.mounts.join(", ")
        )
    }
}

/// Every service in this stack that would see more than one mount beneath the
/// data root.
///
/// Given every compose file at once: a service's mounts are what it declares plus
/// whatever it extends, and the two are commonly in different files.
#[must_use]
pub(crate) fn crowded(files: &[(PathBuf, String)]) -> Vec<Crowded> {
    let declared = declarations(files);
    let mut crowded: Vec<Crowded> = Vec::new();
    for key in declared.keys() {
        let mounts = beneath_the_root(&gather(key, &declared, &mut BTreeSet::new()));
        if mounts.len() > 1 {
            crowded.push(Crowded {
                service: key.service.clone(),
                mounts,
            });
        }
    }
    crowded.sort_by(|left, right| left.service.cmp(&right.service));
    crowded.dedup_by(|left, right| left.service == right.service);
    crowded
}

/// A service, in the file that declares it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    /// The compose file, as the stack names it.
    file: PathBuf,
    /// The service within it.
    service: String,
}

/// What one service declares: its own volumes, and what it extends.
#[derive(Debug, Clone, Default)]
struct Declared {
    /// The volume entries, as written.
    volumes: Vec<String>,
    /// The service it extends, where it extends one.
    extends: Option<Key>,
}

/// Every service in every file, by where it was declared.
fn declarations(files: &[(PathBuf, String)]) -> BTreeMap<Key, Declared> {
    let mut declared = BTreeMap::new();
    for (path, text) in files {
        let Some(services) = services_in(text) else {
            continue;
        };
        for (name, service) in services {
            declared.insert(
                Key {
                    file: path.clone(),
                    service: name,
                },
                Declared {
                    volumes: volumes_of(&service),
                    extends: extends_of(&service, path),
                },
            );
        }
    }
    declared
}

/// The services a compose file declares, or nothing where it is not one this can
/// read.
///
/// Merge keys are applied first, so a stack written with YAML anchors is read the
/// same way Compose reads it rather than as a service with no volumes at all.
fn services_in(text: &str) -> Option<Vec<(String, Value)>> {
    let mut document: Value = serde_yaml_ng::from_str(text).ok()?;
    document.apply_merge().ok()?;
    let services = document.get("services")?.as_mapping()?;
    Some(
        services
            .iter()
            .filter_map(|(name, service)| Some((name.as_str()?.to_owned(), service.clone())))
            .collect(),
    )
}

/// The volume entries one service declares, in either syntax.
///
/// The long form's `source` is the host side; the short form's is everything
/// before the separator. A named volume is not a path beneath anything and is
/// left to be filtered out later, where the test is about the data root rather
/// than about syntax.
fn volumes_of(service: &Value) -> Vec<String> {
    let Some(volumes) = service.get("volumes").and_then(Value::as_sequence) else {
        return Vec::new();
    };
    volumes
        .iter()
        .filter_map(|volume| match volume {
            Value::String(entry) => Some(entry.clone()),
            other => other
                .get("source")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
        .collect()
}

/// What this service extends, where it extends one — the long form naming another
/// file, or the short form naming a service in this one.
fn extends_of(service: &Value, here: &Path) -> Option<Key> {
    let extends = service.get("extends")?;
    if let Some(name) = extends.as_str() {
        return Some(Key {
            file: here.to_path_buf(),
            service: name.to_owned(),
        });
    }
    let service = extends.get("service")?.as_str()?.to_owned();
    let file = extends
        .get("file")
        .and_then(Value::as_str)
        .map_or_else(|| here.to_path_buf(), PathBuf::from);
    Some(Key { file, service })
}

/// Every volume this service ends up with, following what it extends.
///
/// `seen` stops a cycle: a pair of services extending each other is a stack
/// Compose would refuse, and this has no business looping for ever over it.
fn gather(key: &Key, declared: &BTreeMap<Key, Declared>, seen: &mut BTreeSet<Key>) -> Vec<String> {
    if !seen.insert(key.clone()) {
        return Vec::new();
    }
    let Some(service) = declared.get(key).or_else(|| by_name(key, declared)) else {
        return Vec::new();
    };
    let mut volumes = service.volumes.clone();
    if let Some(parent) = &service.extends {
        volumes.extend(gather(parent, declared, seen));
    }
    volumes
}

/// The same service found by name alone.
///
/// An `extends` names a file the way the stack's own includes do, which is not
/// always the path this read the file by. Falling back to the name keeps a stack
/// whose paths are written differently from being read as though it extended
/// nothing — under-reporting is the failure that matters here, since it would
/// call a broken stack clean.
fn by_name<'a>(key: &Key, declared: &'a BTreeMap<Key, Declared>) -> Option<&'a Declared> {
    declared
        .iter()
        .find(|(candidate, _)| {
            candidate.service == key.service && candidate.file.file_name() == key.file.file_name()
        })
        .map(|(_, service)| service)
}

/// The volume entries whose host side sits beneath the data root.
fn beneath_the_root(volumes: &[String]) -> Vec<String> {
    volumes
        .iter()
        .filter(|volume| is_beneath_the_root(host_side(volume)))
        .cloned()
        .collect()
}

/// The host side of a volume entry.
///
/// Split at the first separator *outside* a variable reference: the data root is
/// commonly written `${DATA_ROOT:-./data}`, whose default carries a colon of its
/// own, and splitting naively would leave `${DATA_ROOT` and find nothing.
fn host_side(volume: &str) -> &str {
    let mut depth = 0usize;
    for (at, character) in volume.char_indices() {
        match character {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ':' if depth == 0 => return volume.get(..at).unwrap_or(volume),
            _ => {}
        }
    }
    volume
}

/// Whether a host path is the data root or something inside it.
///
/// Recognised by the variable rather than by a resolved path, because this reads
/// the stack as written — before any environment file exists, and for a stack the
/// operator has not configured yet.
fn is_beneath_the_root(host: &str) -> bool {
    let rest = host
        .strip_prefix("${")
        .or_else(|| host.strip_prefix('$'))
        .unwrap_or("");
    rest.strip_prefix(DATA_ROOT).is_some_and(|after| {
        after
            .chars()
            .next()
            .is_none_or(|next| !next.is_alphanumeric() && next != '_')
    })
}

#[cfg(test)]
mod tests;
