//! The part of the stack this binary carries: what runs, and nothing that only
//! builds, tests or describes the stack's own repository.
//!
//! The submodule is that whole repository — its CI, its scripts, its recordings, its
//! hooks — and an operator's stack directory is written from whatever is carried. So
//! what is carried is chosen here and copied beside this build's other outputs, and
//! the binary embeds that copy:
//!
//! - **Whole:** the manifest and its service files, the compose files and every fragment, the stack
//!   variants, the services' starting configuration, the settings file the
//!   configuration surface reads, and the licence the stack is published under.
//! - **What a compose file names:** a file on the host a service mounts, or reads its
//!   environment from, is carried wherever it lives.
//! - **What a claim names:** every recording the manifest's claims are proven
//!   against. A recording it names and the stack does not hold refuses the build,
//!   because a claim nothing can prove is a stack that cannot be judged.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

/// The files carried whole, relative to the stack's root.
const WHOLE_FILES: [&str; 4] = ["stack.toml", "compose.yml", ".env.example", "LICENSE"];

/// The directories carried whole, relative to the stack's root.
const WHOLE_DIRECTORIES: [&str; 4] = ["services", "compose", "stacks", "config"];

/// What a reference to a file beside the compose file starts with.
const BESIDE: &str = "./";

/// The extensions a compose file is written with.
const COMPOSED: [&str; 2] = ["yml", "yaml"];

/// Copy what runs of the stack at `root` into `out/stack`.
pub fn stack(root: &Path, out: &Path) {
    println!("cargo::rerun-if-changed={}", root.display());
    let into = out.join("stack");
    let _ = std::fs::remove_dir_all(&into);
    for relative in runtime(root) {
        let target = into.join(&relative);
        let made = target
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::copy(root.join(&relative), &target));
        if let Err(error) = made {
            super::refuse(&[
                &format!("{} could not be carried: {error}", relative.display()),
                &format!("Into: {}", into.display()),
            ]);
        }
    }
}

/// Every file of the stack at `root` that runs, relative to it.
fn runtime(root: &Path) -> BTreeSet<PathBuf> {
    let mut carried: BTreeSet<PathBuf> = WHOLE_FILES
        .iter()
        .map(PathBuf::from)
        .filter(|relative| root.join(relative).is_file())
        .collect();
    for directory in WHOLE_DIRECTORIES {
        within(root, Path::new(directory), &mut carried);
    }
    let composed: Vec<PathBuf> = carried
        .iter()
        .filter(|relative| {
            relative
                .extension()
                .is_some_and(|extension| COMPOSED.iter().any(|composed| extension == *composed))
        })
        .cloned()
        .collect();
    for file in composed {
        let text = std::fs::read_to_string(root.join(&file)).unwrap_or_default();
        let named = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text)
            .map(|document| mounted(&document))
            .unwrap_or_default();
        carried.extend(
            named
                .iter()
                .filter_map(|source| source.strip_prefix(BESIDE))
                .map(PathBuf::from)
                .filter(|relative| beneath(relative) && root.join(relative).is_file()),
        );
    }
    carried.extend(proven(root));
    carried
}

/// Whether a path named relative to the stack's root stays beneath it: names only,
/// with nothing that climbs out of it or starts again from the top.
fn beneath(relative: &Path) -> bool {
    relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
}

/// Every file beneath `directory` of the stack at `root`, relative to the root.
fn within(root: &Path, directory: &Path, into: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root.join(directory)) else {
        return;
    };
    for entry in entries.flatten() {
        let relative = directory.join(entry.file_name());
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => within(root, &relative, into),
            Ok(kind) if kind.is_file() => {
                into.insert(relative);
            }
            _ => {}
        }
    }
}

/// Every host path a compose document names for a service to mount or read: each
/// volume's source, in either of the two ways Compose takes one, and each
/// environment file.
fn mounted(document: &serde_yaml_ng::Value) -> Vec<String> {
    let mut named = Vec::new();
    let Some(services) = document
        .get("services")
        .and_then(serde_yaml_ng::Value::as_mapping)
    else {
        return named;
    };
    for service in services.values() {
        for volume in listed(service.get("volumes")) {
            match volume {
                serde_yaml_ng::Value::String(short) => {
                    named.extend(short.split(':').next().map(str::to_owned));
                }
                long => named.extend(
                    long.get("source")
                        .and_then(|source| source.as_str())
                        .map(str::to_owned),
                ),
            }
        }
        for file in listed(service.get("env_file")) {
            named.extend(file.as_str().map(str::to_owned));
        }
    }
    named
}

/// A field Compose takes as one value or as a list of them, as a list.
fn listed(field: Option<&serde_yaml_ng::Value>) -> Vec<&serde_yaml_ng::Value> {
    match field {
        Some(serde_yaml_ng::Value::Sequence(each)) => each.iter().collect(),
        Some(one) => vec![one],
        None => Vec::new(),
    }
}

/// Every recording the manifest's claims are proven against, relative to the root.
fn proven(root: &Path) -> BTreeSet<PathBuf> {
    let manifest = lemonfiber_manifest::read(root).unwrap_or_default();
    let mut named = Vec::new();
    if let Ok(document) = manifest.parse::<toml::Table>() {
        fixtures(&toml::Value::Table(document), &mut named);
    }
    let missing: Vec<&String> = named
        .iter()
        .filter(|fixture| !beneath(Path::new(fixture)) || !root.join(fixture).is_file())
        .collect();
    if !missing.is_empty() {
        let mut said = vec!["the stack's claims name recordings it does not hold".to_owned()];
        said.extend(missing.iter().map(|fixture| format!("Missing: {fixture}")));
        let lines: Vec<&str> = said.iter().map(String::as_str).collect();
        super::refuse(&lines);
    }
    named.into_iter().map(PathBuf::from).collect()
}

/// Every value under a key named `fixture`, anywhere in a TOML document.
fn fixtures(value: &toml::Value, into: &mut Vec<String>) {
    match value {
        toml::Value::Table(table) => {
            for (key, inner) in table {
                match (key.as_str(), inner) {
                    ("fixture", toml::Value::String(fixture)) => into.push(fixture.clone()),
                    _ => fixtures(inner, into),
                }
            }
        }
        toml::Value::Array(each) => each.iter().for_each(|inner| fixtures(inner, into)),
        _ => {}
    }
}
