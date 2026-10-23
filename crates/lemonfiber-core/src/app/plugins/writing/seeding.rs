//! Writing a plugin's starting files and an adapter's `upstream.json`, each secret they
//! name filled from the secret store, beneath the stack directory through no link.

use std::path::Path;

use crate::error::Problem;
use crate::ports::filesystem::Readable;

use super::super::super::Ctx;
use super::{recorded_making, unwritable, LINKED};

/// Write a starting file or an adapter's `upstream.json`, each secret it names written
/// as the secret store holds it for `plugin`.
pub(super) fn seeded(
    ctx: &Ctx,
    recording: (&str, &str, &Path),
    write: &crate::plugin::Write,
) -> Result<(), Box<Problem>> {
    let plugin = recording.0;
    let secrets = crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .unwrap_or_default();
    let held = |secret: &str| {
        secrets
            .value(&crate::settings::named(plugin, secret))
            .map(str::to_owned)
    };
    let unwritten = |why: String| Box::new(unwritable(&write.path, &why));
    match &write.lands {
        crate::plugin::Lands::Starting { bytes, seeded } => {
            let bytes = crate::plugin::seeding::written(bytes, &held, ctx.seams.random.as_ref())
                .map_err(unwritten)?;
            let readable = if *seeded {
                Readable::Owner
            } else {
                Readable::Shared
            };
            made_confined(ctx, recording, &write.path, &bytes, readable)
        }
        crate::plugin::Lands::Upstream(fields) => {
            let text = crate::plugin::seeding::upstream(fields, &held).map_err(unwritten)?;
            made_confined(
                ctx,
                recording,
                &write.path,
                text.as_bytes(),
                Readable::Owner,
            )
        }
        crate::plugin::Lands::Directory { .. }
        | crate::plugin::Lands::Document { .. }
        | crate::plugin::Lands::Key
        | crate::plugin::Lands::Secrets(_)
        | crate::plugin::Lands::Region { .. } => Ok(()),
    }
}

/// Write one file beneath the stack directory that a plugin's service reads, readable as
/// `readable` says, through no link on the way to it, since the directory is one its
/// container writes.
fn made_confined(
    ctx: &Ctx,
    recording: (&str, &str, &Path),
    path: &Path,
    content: &[u8],
    readable: Readable,
) -> Result<(), Box<Problem>> {
    let stack = super::super::super::project::directory(ctx)
        .ok_or_else(|| Box::new(unwritable(path, LINKED)))?;
    recorded_making(ctx, recording, path, false)?;
    ctx.seams
        .confined
        .written(path, &stack, content, readable)
        .map_err(|fault| Box::new(unwritable(path, &fault.message)))
}
