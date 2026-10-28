//! Each stack service's further mount sources, made in its mount tree before a start
//! where nothing stands there yet.
//!
//! The engine makes a bind mount's missing source itself, owned by root, and a service
//! that runs as the operator cannot then write what it was given. So lemonfiber makes
//! each one first, as the operator who runs it, journalled as made so a reversal takes it
//! away, the way an install makes a plugin's mount tree. What already stands there is
//! left as it is, and so is anything reached through a link.

use std::path::{Path, PathBuf};

use lemonfiber_manifest::Manifest;

use super::super::Ctx;
use super::moving::{change, missing, shown};
use crate::error::{Diagnose, Problem};
use crate::journal::Kind;
use crate::plugin::Installed;
use crate::ports::filesystem::{Lying, Readable};
use crate::project::entry::{mounts, MOUNT_TREE};
use crate::stack::Failure;

/// Make every further mount source not there yet of each stack service the project
/// writes beside `installed`, journalling each directory it makes first.
///
/// # Errors
///
/// Where the journal cannot be written, or a directory cannot be made.
pub(crate) fn made(
    ctx: &Ctx,
    manifest: &Manifest,
    installed: &[Installed],
) -> Result<(), Box<Problem>> {
    let (false, Some(directory), Some(paths)) = (
        ctx.dry_run,
        super::directory(ctx),
        super::super::targets::layout(ctx),
    ) else {
        return Ok(());
    };
    let stamp = ctx.stamp();
    for source in absent(ctx, &directory, manifest, installed) {
        let changes: Vec<_> = missing(&source)
            .into_iter()
            .chain(std::iter::once(source.clone()))
            .map(|path| change(&stamp, Kind::Made { path: shown(&path) }, &path))
            .collect();
        crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())
            .map_err(|failure| Box::new(failure.problem()))?;
        ctx.seams
            .confined
            .made(&source, &directory, Readable::Shared)
            .map_err(|fault| {
                Box::new(
                    Failure::NotWritten {
                        path: source.clone(),
                        reason: fault.message,
                    }
                    .problem(),
                )
            })?;
    }
    Ok(())
}

/// Every further mount source beneath `directory` of the stack services the project
/// writes beside `installed` that is not there, reached through no link.
fn absent(
    ctx: &Ctx,
    directory: &Path,
    manifest: &Manifest,
    installed: &[Installed],
) -> Vec<PathBuf> {
    crate::project::written(manifest, installed)
        .flat_map(|service| {
            mounts(&service.mount, service.takes_data).map(move |mount| {
                directory
                    .join(MOUNT_TREE)
                    .join(&service.id)
                    .join(&mount.from)
            })
        })
        .filter(|source| ctx.seams.confined.lies(source, directory) == Lying::Absent)
        .collect()
}
