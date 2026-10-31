//! The stack a build carries: what runs, and nothing that only builds, tests or
//! describes the repositories it is published from.
//!
//! An operator's stack directory is written from whatever is carried, so what is
//! carried is chosen here and written beside this build's other outputs, and that copy
//! is what is embedded and what every test of the shipped stack reads:
//!
//! - **Never the manifest:** the stack the bundle declares is handed over whole, beside
//!   this tree, and no root or service file of a stack directory is carried.
//! - **Each assembled part's starting configuration:** from its copy in the bundle, which
//!   holds its tree and nothing beside it. A part the bundle installs is not carried: the
//!   plugin path writes its starting files where none is yet, and its adapter owns them
//!   from then on.
//! - **The licence the stack is published under:** the catalogue's.
//! - **What a claim names:** every recording the manifest's claims are proven against,
//!   from the copy of the part whose recording it is. A recording a claim names and no
//!   copy holds refuses the build, because a claim nothing can prove is a stack that
//!   cannot be judged.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use lemonfiber_bundle::Held;

/// The licence the stack is published under, relative to the catalogue's root.
pub const LICENCE: &str = "LICENSE";

/// Where a part's starting configuration is, relative to a stack's root or its copy's.
const CONFIG: &str = "config";

/// Write what runs of the stack `text` the bundle declares into `out/stack`: the copies
/// of the parts `held` from `copies`, and the licence from the catalogue at `catalogue`,
/// and never the manifest, which the crate hands over whole as `STACK`.
pub fn stack(catalogue: &Path, copies: &Path, text: &str, held: &[Held], out: &Path) {
    let into = out.join(super::CARRIED);
    let _ = std::fs::remove_dir_all(&into);
    for (relative, from) in runtime(catalogue, copies, text, held) {
        let target = into.join(&relative);
        let made = target
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::copy(&from, &target).map(|_| ()));
        if let Err(error) = made {
            super::refuse(
                &format!("{} could not be carried", relative.display()),
                &error.to_string(),
                &into,
            );
        }
    }
}

/// Every file of the stack that runs, by its path in a stack directory, with where its
/// bytes come from.
fn runtime(
    catalogue: &Path,
    copies: &Path,
    text: &str,
    held: &[Held],
) -> BTreeMap<PathBuf, PathBuf> {
    let mut carried = BTreeMap::new();
    if catalogue.join(LICENCE).is_file() {
        carried.insert(PathBuf::from(LICENCE), catalogue.join(LICENCE));
    }
    for one in held
        .iter()
        .filter(|one| one.pin.stage != Some(lemonfiber_bundle::Stage::Installed))
    {
        let id = &one.pin.id;
        from(&copies.join(id), &Path::new(CONFIG).join(id), &mut carried);
    }
    carried.extend(proven(copies, text));
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
fn from(root: &Path, directory: &Path, into: &mut BTreeMap<PathBuf, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root.join(directory)) else {
        return;
    };
    for entry in entries.flatten() {
        let relative = directory.join(entry.file_name());
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => from(root, &relative, into),
            Ok(kind) if kind.is_file() => {
                into.insert(relative.clone(), root.join(relative));
            }
            _ => {}
        }
    }
}

/// Every recording the manifest's claims are proven against, by its path in a stack
/// directory, from the copy of the part whose recordings it is.
fn proven(copies: &Path, text: &str) -> BTreeMap<PathBuf, PathBuf> {
    let mut named = Vec::new();
    if let Ok(document) = text.parse::<toml::Table>() {
        fixtures(&toml::Value::Table(document), &mut named);
    }
    let mut proven = BTreeMap::new();
    let mut missing = Vec::new();
    for fixture in named {
        let relative = PathBuf::from(&fixture);
        let held = relative
            .components()
            .nth(1)
            .and_then(|part| part.as_os_str().to_str())
            .map(|id| copies.join(id).join(&relative))
            .filter(|path| beneath(&relative) && path.is_file());
        match held {
            Some(path) => {
                proven.insert(relative, path);
            }
            None => missing.push(fixture),
        }
    }
    if !missing.is_empty() {
        let said: Vec<String> = missing
            .iter()
            .map(|fixture| format!("Missing: {fixture}"))
            .collect();
        super::refuse(
            "the stack's claims name recordings it does not hold",
            &said.join("\n"),
            copies,
        );
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
