//! The stack this binary carries: what runs, and nothing that only builds, tests or
//! describes the repositories it is published from.
//!
//! An operator's stack directory is written from whatever is carried, so what is
//! carried is chosen here and written beside this build's other outputs, and the
//! binary embeds that copy:
//!
//! - **The manifest:** the stack the bundle declares, written apart as a root and a
//!   file per service, as a stack directory holds it.
//! - **Each part's starting configuration:** an assembled part's from its copy in the
//!   bundle, which holds its tree and nothing beside it; an installed part's from the
//!   media stack, which declares it until it moves.
//! - **What the media stack publishes beside it:** the settings file the configuration
//!   surface reads and the licence it is published under.
//! - **What a claim names:** every recording the manifest's claims are proven against,
//!   from the part's copy where it holds it. A recording a claim names and nothing
//!   holds refuses the build, because a claim nothing can prove is a stack that
//!   cannot be judged.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use lemonfiber_bundled::{Stage, PINNED, STACK};

/// The licence the media stack is published under, relative to its root.
pub const LICENCE: &str = "LICENSE";

/// The media stack's files carried whole, relative to its root.
const WHOLE_FILES: [&str; 2] = [".env.example", LICENCE];

/// Where a part's starting configuration is, relative to a stack's root or its copy's.
const CONFIG: &str = "config";

/// Where a carried file's bytes come from.
enum Carried {
    /// A file on disk, in full.
    File(PathBuf),
    /// Text this build wrote.
    Text(String),
}

/// Write what runs of the stack the bundle declares into `out/stack`: the parts'
/// copies from `copies`, and the rest from the media stack at `media`.
pub fn stack(media: &Path, copies: &Path, out: &Path) {
    println!("cargo::rerun-if-changed={}", media.display());
    println!("cargo::rerun-if-changed={}", copies.display());
    let into = out.join("stack");
    let _ = std::fs::remove_dir_all(&into);
    for (relative, from) in runtime(media, copies) {
        let target = into.join(&relative);
        let made = target
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| match &from {
                Carried::File(path) => std::fs::copy(path, &target).map(|_| ()),
                Carried::Text(text) => std::fs::write(&target, text),
            });
        if let Err(error) = made {
            super::refuse(&[
                &format!("{} could not be carried: {error}", relative.display()),
                &format!("Into: {}", into.display()),
            ]);
        }
    }
}

/// Every file of the stack that runs, by its path in a stack directory, with where its
/// bytes come from.
fn runtime(media: &Path, copies: &Path) -> BTreeMap<PathBuf, Carried> {
    let mut carried = manifest();
    for relative in WHOLE_FILES.iter().map(PathBuf::from) {
        if media.join(&relative).is_file() {
            carried.insert(relative.clone(), Carried::File(media.join(relative)));
        }
    }
    for pinned in PINNED {
        let id = pinned.first_party.plugin;
        let starting = Path::new(CONFIG).join(id);
        match pinned.stage {
            Some(Stage::Assembled) => from(&copies.join(id), &starting, &mut carried),
            _ => from(media, &starting, &mut carried),
        }
    }
    carried.extend(proven(media, copies));
    carried
}

/// The stack the bundle declares, written apart as a stack directory holds it.
fn manifest() -> BTreeMap<PathBuf, Carried> {
    let Ok((root, services)) = lemonfiber_manifest::assembly::split(STACK) else {
        super::refuse(&["the stack the bundle declares cannot be written apart"]);
    };
    let mut carried = BTreeMap::from([(
        PathBuf::from(lemonfiber_manifest::assembly::ROOT),
        Carried::Text(root),
    )]);
    for (file, text) in services {
        carried.insert(
            Path::new(lemonfiber_manifest::assembly::SERVICES).join(file),
            Carried::Text(text),
        );
    }
    carried
}

/// Whether a path named relative to the stack's root stays beneath it: names only,
/// with nothing that climbs out of it or starts again from the top.
fn beneath(relative: &Path) -> bool {
    relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
}

/// Every file beneath `directory` of the tree at `root`, by its path relative to the
/// root, carried from there.
fn from(root: &Path, directory: &Path, into: &mut BTreeMap<PathBuf, Carried>) {
    let Ok(entries) = std::fs::read_dir(root.join(directory)) else {
        return;
    };
    for entry in entries.flatten() {
        let relative = directory.join(entry.file_name());
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => from(root, &relative, into),
            Ok(kind) if kind.is_file() => {
                into.insert(relative.clone(), Carried::File(root.join(relative)));
            }
            _ => {}
        }
    }
}

/// Every recording the manifest's claims are proven against, by its path in a stack
/// directory: from the copy of the part whose recordings it is where the copy holds it,
/// and from the media stack at `media` otherwise.
fn proven(media: &Path, copies: &Path) -> BTreeMap<PathBuf, Carried> {
    let mut named = Vec::new();
    if let Ok(document) = STACK.parse::<toml::Table>() {
        fixtures(&toml::Value::Table(document), &mut named);
    }
    let mut proven = BTreeMap::new();
    let mut missing = Vec::new();
    for fixture in named {
        let relative = PathBuf::from(&fixture);
        let in_copy = relative
            .components()
            .nth(1)
            .and_then(|part| part.as_os_str().to_str())
            .map(|id| copies.join(id).join(&relative));
        let held = in_copy
            .into_iter()
            .chain([media.join(&relative)])
            .find(|path| beneath(&relative) && path.is_file());
        match held {
            Some(path) => {
                proven.insert(relative, Carried::File(path));
            }
            None => missing.push(fixture),
        }
    }
    if !missing.is_empty() {
        let mut said = vec!["the stack's claims name recordings it does not hold".to_owned()];
        said.extend(missing.iter().map(|fixture| format!("Missing: {fixture}")));
        let lines: Vec<&str> = said.iter().map(String::as_str).collect();
        super::refuse(&lines);
    }
    proven
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
