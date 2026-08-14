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
pub fn crowded(files: &[(PathBuf, String)]) -> Vec<Crowded> {
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
mod tests {
    use super::{crowded, host_side, is_beneath_the_root};
    use std::path::PathBuf;

    /// The stack's compose files, as the check is given them.
    fn given(files: &[(&str, &str)]) -> Vec<(PathBuf, String)> {
        files
            .iter()
            .map(|(path, text)| (PathBuf::from(path), (*text).to_owned()))
            .collect()
    }

    /// A one-service compose file declaring the given volume entries.
    ///
    /// Built rather than written out, so the clean cases and the refused ones
    /// share a shape: a builder that stopped describing a compose file would fail
    /// the tests that expect a refusal instead of quietly passing the ones that
    /// expect none.
    fn service_with(volumes: &[&str]) -> String {
        let mut text = String::from("services:\n  sonarr:\n    volumes:\n");
        for volume in volumes {
            text.push_str("      - ");
            text.push_str(volume);
            text.push('\n');
        }
        text
    }

    /// The services reported crowded in a single built file.
    fn crowding(volumes: &[&str]) -> Vec<String> {
        named(&[("tv.yml", &service_with(volumes))])
    }

    /// The services reported crowded, by name.
    fn named(files: &[(&str, &str)]) -> Vec<String> {
        crowded(&given(files))
            .into_iter()
            .map(|crowded| crowded.service)
            .collect()
    }

    #[test]
    fn the_sanctioned_shape_passes() {
        // One mount of the data root, and a configuration directory that is not
        // beneath it. This is what every service in the shipped stack looks like.
        assert!(crowding(&["${DATA_ROOT:-./data}:/data", "./config/sonarr:/config"]).is_empty());
    }

    #[test]
    fn two_mounts_beneath_the_root_are_refused() {
        // The tidier-looking form, and the one that silently turns every import
        // into a copy: /downloads and /media are different filesystems inside the
        // container, so nothing can be linked between them.
        assert_eq!(
            crowding(&[
                "${DATA_ROOT}/downloads:/downloads",
                "${DATA_ROOT}/media:/media"
            ]),
            vec!["sonarr".to_owned()]
        );
    }

    #[test]
    fn what_it_costs_is_named_rather_than_the_rule_it_broke() {
        // Naming the rule it broke tells an operator nothing. What they need is
        // what will happen to them, and which paths did it.
        let file = service_with(&[
            "${DATA_ROOT}/downloads:/downloads",
            "${DATA_ROOT}/media:/media",
        ]);
        let said = crowded(&given(&[("tv.yml", &file)]))
            .first()
            .map(ToString::to_string)
            .unwrap_or_default();
        assert!(said.contains("copied rather than hardlinked"), "{said}");
        assert!(said.contains("${DATA_ROOT}/media:/media"), "{said}");
    }

    #[test]
    fn two_services_with_one_mount_each_are_the_required_form() {
        // Every service mounting the whole data root once is exactly right, and a
        // rule counted across the stack rather than per service would refuse it.
        let stack = "
services:
  sonarr:
    volumes:
      - ${DATA_ROOT}:/data
  qbittorrent:
    volumes:
      - ${DATA_ROOT}:/data
";
        assert!(named(&[("media.yml", stack)]).is_empty());
    }

    #[test]
    fn a_mount_inherited_through_extends_still_counts() {
        // The case a file-at-a-time reading calls clean: one mount here, one in
        // what it extends, and two in the container that actually runs.
        let common = "
services:
  defaults:
    volumes:
      - ${DATA_ROOT}/downloads:/downloads
";
        let tv = "
services:
  sonarr:
    extends:
      file: compose/_common.yml
      service: defaults
    volumes:
      - ${DATA_ROOT}/media:/media
";
        assert_eq!(
            named(&[("compose/_common.yml", common), ("compose/tv.yml", tv)]),
            vec!["sonarr".to_owned()]
        );
    }

    #[test]
    fn a_service_extending_one_in_its_own_file_is_followed_too() {
        let stack = "
services:
  base:
    volumes:
      - ${DATA_ROOT}/downloads:/downloads
  sonarr:
    extends: base
    volumes:
      - ${DATA_ROOT}/media:/media
";
        assert_eq!(named(&[("tv.yml", stack)]), vec!["sonarr".to_owned()]);
    }

    #[test]
    fn services_extending_each_other_are_refused_rather_than_looped_over() {
        // Compose would refuse this stack; this has no business hanging on it.
        let cycle = "
services:
  one:
    extends: two
    volumes:
      - ${DATA_ROOT}/a:/a
  two:
    extends: one
    volumes:
      - ${DATA_ROOT}/b:/b
";
        assert_eq!(
            named(&[("loop.yml", cycle)]),
            vec!["one".to_owned(), "two".to_owned()]
        );
    }

    #[test]
    fn an_anchor_is_read_the_way_compose_reads_it() {
        // A stack written with YAML anchors rather than `extends`. Without the
        // merge applied, the service would read as having no volumes at all and
        // this check would call it clean.
        let anchored = "
x-shared: &shared
  volumes:
    - ${DATA_ROOT}/downloads:/downloads
    - ${DATA_ROOT}/media:/media
services:
  sonarr:
    <<: *shared
";
        assert_eq!(named(&[("tv.yml", anchored)]), vec!["sonarr".to_owned()]);
    }

    #[test]
    fn the_long_form_is_read_as_well_as_the_short() {
        let long = "
services:
  sonarr:
    volumes:
      - type: bind
        source: ${DATA_ROOT}/downloads
        target: /downloads
      - type: bind
        source: ${DATA_ROOT}/media
        target: /media
";
        assert_eq!(named(&[("tv.yml", long)]), vec!["sonarr".to_owned()]);
    }

    #[test]
    fn a_named_volume_is_not_a_path_beneath_anything() {
        // Caddy's certificate store, and the reason the test is on the data root
        // rather than on how many volumes a service has.
        assert!(crowding(&["caddy_data:/data", "${DATA_ROOT}:/srv"]).is_empty());
        // And the same file with a second path beneath the root is refused, so a
        // pass above cannot come from a parser that read nothing.
        assert_eq!(
            crowding(&["caddy_data:/data", "${DATA_ROOT}:/srv", "${DATA_ROOT}/x:/x"]),
            vec!["sonarr".to_owned()]
        );
    }

    #[test]
    fn a_variable_that_merely_starts_the_same_is_not_the_data_root() {
        // DATA_ROOT_BACKUP is a different location, and reading it as the data
        // root would refuse a stack that is doing nothing wrong.
        assert!(is_beneath_the_root("${DATA_ROOT}"));
        assert!(is_beneath_the_root("${DATA_ROOT:-./data}/media"));
        assert!(is_beneath_the_root("$DATA_ROOT/media"));
        assert!(!is_beneath_the_root("${DATA_ROOT_BACKUP}/media"));
        assert!(!is_beneath_the_root("./config/sonarr"));
        assert!(!is_beneath_the_root("caddy_data"));
    }

    #[test]
    fn a_default_carrying_a_colon_does_not_cut_the_host_path_short() {
        // `${DATA_ROOT:-./data}` has a colon of its own, and splitting at the
        // first one leaves `${DATA_ROOT` — which matches nothing, so every stack
        // written the shipped way would read as clean.
        assert_eq!(
            host_side("${DATA_ROOT:-./data}:/data"),
            "${DATA_ROOT:-./data}"
        );
        assert_eq!(host_side("./config:/config:ro"), "./config");
        assert_eq!(host_side("caddy_data"), "caddy_data");
    }

    #[test]
    fn a_file_that_is_not_a_compose_file_is_passed_over() {
        // Stack directories hold licences, readmes and justfiles. A parser given
        // one must not refuse the stack over it.
        assert!(named(&[("README.md", "# not yaml: [")]).is_empty());
        assert!(named(&[("stack.toml", "schema_version = 1")]).is_empty());
    }

    #[test]
    fn a_service_that_mounts_nothing_is_not_reported() {
        let bare = "
services:
  watchtower:
    image: containrrr/watchtower
";
        assert!(named(&[("tuning.yml", bare)]).is_empty());
    }

    #[test]
    fn extending_something_no_file_declares_adds_nothing_rather_than_a_guess() {
        // A stack can name a parent this never sees — a typo, or a file outside
        // what was handed here. What it declares itself still counts; what it
        // extends contributes nothing, because inventing volumes for it would
        // refuse a stack over something nobody wrote.
        let orphan = "
services:
  sonarr:
    extends:
      file: somewhere/else.yml
      service: nothing-declares-this
    volumes:
      - ${DATA_ROOT}:/data
";
        assert!(named(&[("tv.yml", orphan)]).is_empty());
    }
}
