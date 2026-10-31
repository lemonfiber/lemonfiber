//! The stack's own Compose files, which nothing reads once the project is generated, and
//! the manifest an earlier release wrote out for the embedded stack, which nothing reads
//! once that stack is read where it is carried.
//!
//! Each one lemonfiber wrote and nobody changed since is taken away, and so is every
//! document it wrote for a plugin and the file standing beside an existing setup was
//! once layered with; one somebody changed is left where it is, for the doctor to name.

use std::path::{Path, PathBuf};

use super::super::Ctx;
use crate::materialised::{checksum, Materialised};

/// The stack's own Compose file, beneath the project directory.
const COMPOSE_FILE: &str = "compose.yml";

/// The directory of the stack's own Compose fragments, beneath the project directory.
const FRAGMENTS: &str = "compose";

/// The directory of the stack's own variants, beneath the project directory.
const VARIANTS: &str = "stacks";

/// Where the stack kept its own Compose files, beneath the project directory.
const STATIC: [&str; 3] = [COMPOSE_FILE, FRAGMENTS, VARIANTS];

/// The directory beneath the fragments where each plugin's own document was kept.
const PLUGIN_DOCUMENTS: &str = "plugins";

/// The Compose file standing beside an existing setup was layered with, beside the
/// settings.
const BESIDE: &str = "beside.yml";

/// What a manifest file is named with.
const MANIFESTED: &str = "toml";

/// What a file Compose reads is named with.
const COMPOSED: [&str; 2] = ["yml", "yaml"];

/// Take away every static Compose file and manifest file lemonfiber wrote into the
/// embedded stack and nobody changed, every plugin's document, and the file standing
/// beside was layered with. Best effort: a file that will not go is one nothing reads.
pub(crate) fn retired(ctx: &Ctx, installed: &[crate::plugin::Installed]) {
    let ours = !ctx.dry_run && !matches!(ctx.stack, crate::stack::Source::External(_));
    let Some(directory) = super::directory(ctx).filter(|_| ours) else {
        return;
    };
    let at = super::record_path(ctx);
    let mut record: Materialised = super::super::record::kept(at.as_deref());
    let keys: Vec<String> = record
        .paths()
        .filter(|key| is_static(Path::new(key)) || is_manifest(Path::new(key)))
        .map(str::to_owned)
        .collect();
    for key in &keys {
        let path = directory.join(key);
        let held = crate::within::read_unlinked(ctx.seams.confined.as_ref(), &path, &directory);
        match held {
            Ok(None) => record.forget(key),
            Ok(Some(text))
                if Some(checksum(text.as_bytes())) == record.checksum(key)
                    && std::fs::remove_file(&path).is_ok() =>
            {
                record.forget(key);
            }
            _ => {}
        }
    }
    let _ = super::super::record::keep(at.as_deref(), &record);
    let documents = directory.join(FRAGMENTS).join(PLUGIN_DOCUMENTS);
    for one in installed {
        let _ = std::fs::remove_file(documents.join(format!("{}.yml", one.plugin)));
    }
    for emptied in [
        documents,
        directory.join(FRAGMENTS),
        directory.join(VARIANTS),
        directory.join(lemonfiber_manifest::assembly::SERVICES),
    ] {
        let _ = std::fs::remove_dir(emptied);
    }
    if let Some(env) = ctx.settings.env_file.as_deref() {
        let _ = std::fs::remove_file(env.with_file_name(BESIDE));
    }
}

/// Every static Compose file still beneath `directory`, which nothing reads.
pub(crate) fn still_there(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending: Vec<PathBuf> = STATIC.iter().map(|one| directory.join(one)).collect();
    while let Some(here) = pending.pop() {
        let Ok(kind) = std::fs::symlink_metadata(&here).map(|meta| meta.file_type()) else {
            continue;
        };
        if kind.is_dir() {
            pending.extend(
                std::fs::read_dir(&here)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|entry| entry.path()),
            );
        } else if kind.is_file() && composed(&here) {
            found.push(here);
        }
    }
    found.sort();
    found
}

/// Whether a path beneath the project directory is where the stack kept a static
/// Compose file.
fn is_static(relative: &Path) -> bool {
    STATIC.iter().any(|one| relative.starts_with(one)) && composed(relative)
}

/// Whether a path beneath the project directory is where an earlier release wrote the
/// embedded stack's manifest: its root, or a file of one of its services.
fn is_manifest(relative: &Path) -> bool {
    relative == Path::new(lemonfiber_manifest::assembly::ROOT)
        || (relative.starts_with(lemonfiber_manifest::assembly::SERVICES)
            && relative
                .extension()
                .is_some_and(|extension| extension == MANIFESTED))
}

/// Whether a path is a file Compose reads.
fn composed(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| {
            COMPOSED
                .iter()
                .any(|one| extension.eq_ignore_ascii_case(one))
        })
}
