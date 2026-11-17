//! What a service's entry mounts: the library where it asked for it, its own configuration
//! directory, the directory of the service it fronts, and each path from its own mount tree,
//! each only where the model allows it.

use lemonfiber_manifest::container::{
    is_source, is_target, lies_inside, shares_a_source, Mount, CONFIGURATION,
};

use super::{
    literal, operated, Unit, Values, CONFIGURATION_TREE, LIBRARY, LIBRARY_TARGET, LIBRARY_VOLUME,
    MOUNT_TREE,
};
use crate::config::DATA_ROOT_KEY;

/// The library where it asked for it, its own configuration directory, and each path
/// mounted from its own mount tree.
pub(super) fn volumes(unit: &Unit<'_>, values: &Values<'_>) -> Vec<String> {
    let service = literal(unit.id);
    let mut volumes = Vec::new();
    if unit.takes_data {
        let from = match values.nfs {
            Some(_) => LIBRARY_VOLUME.to_owned(),
            None => operated(values, DATA_ROOT_KEY, LIBRARY),
        };
        volumes.push(format!("{from}:{LIBRARY_TARGET}"));
    }
    volumes.extend(unit.config_path.map(|path| {
        mounted(
            &format!("./{CONFIGURATION_TREE}/{service}"),
            configuration(path),
            unit.config_ro,
        )
    }));
    if let Some((fronted, to)) = unit.shares.filter(|(_, to)| is_target(to, unit.takes_data)) {
        volumes.push(mounted(
            &format!("./{CONFIGURATION_TREE}/{}", literal(fronted)),
            to,
            false,
        ));
    }
    volumes.extend(
        mounts(unit.mounts, unit.takes_data).map(|Mount { from, to, ro }| {
            mounted(
                &format!("./{MOUNT_TREE}/{service}/{}", literal(from)),
                to,
                *ro,
            )
        }),
    );
    volumes
}

/// Where a configuration directory lands: where it is declared, where that is a target
/// the model allows, and the model's own default otherwise.
pub(crate) fn configuration(declared: &str) -> &str {
    if is_target(declared, true) {
        declared
    } else {
        CONFIGURATION
    }
}

/// Each further mount the model allows: a source inside its own mount tree, inside no
/// other of its sources and the source of no other mount, landing where a mount may land.
pub(crate) fn mounts(all: &[Mount], takes_data: bool) -> impl Iterator<Item = &Mount> {
    all.iter()
        .enumerate()
        .filter(move |(index, mount)| {
            is_source(&mount.from)
                && is_target(&mount.to, takes_data)
                && !shares_a_source(all, *index)
                && !all
                    .iter()
                    .any(|other| lies_inside(&mount.from, &other.from))
        })
        .map(|(_, mount)| mount)
}

/// Each scratch path in memory the model allows.
pub(crate) fn tmpfs(all: &[String], takes_data: bool) -> impl Iterator<Item = &String> {
    all.iter().filter(move |path| is_target(path, takes_data))
}

/// One bind mount of `source` at `to`.
fn mounted(source: &str, to: &str, ro: bool) -> String {
    let mode = if ro { ":ro" } else { "" };
    format!("{source}:{}{mode}", literal(to))
}
