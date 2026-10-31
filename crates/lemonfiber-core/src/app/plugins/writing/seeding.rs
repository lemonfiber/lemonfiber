//! Writing a plugin's starting files and an adapter's `upstream.json`, each secret they
//! name filled from the secret store, beneath the stack directory through no link.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::error::Problem;
use crate::ports::filesystem::Readable;

use super::super::super::Ctx;
use super::{recorded_making, unwritable};

/// Write a starting file holding `bytes`, each secret it names written as the secret store
/// holds it for the plugin, readable by its owner alone where it names one.
pub(super) fn starting(
    ctx: &Ctx,
    recording: (&str, &str, &Path),
    stack: &Path,
    path: &Path,
    bytes: &[u8],
    seeded: bool,
) -> Result<(), Box<Problem>> {
    let bytes = crate::plugin::seeding::written(
        bytes,
        &holding(ctx, recording.0),
        ctx.seams.random.as_ref(),
    )
    .map_err(|why| Box::new(unwritable(path, &why)))?;
    let readable = if seeded {
        Readable::Owner
    } else {
        Readable::Shared
    };
    made_confined(ctx, recording, stack, path, &bytes, readable)
}

/// Write an adapter's `upstream.json`, each of `fields` with its secret's value as the
/// secret store holds it for the plugin, a `lapsing` code it holds none of left out.
pub(super) fn upstream(
    ctx: &Ctx,
    recording: (&str, &str, &Path),
    stack: &Path,
    path: &Path,
    (fields, lapsing): (&BTreeMap<String, String>, &BTreeSet<String>),
) -> Result<(), Box<Problem>> {
    let text = crate::plugin::seeding::upstream(fields, lapsing, &holding(ctx, recording.0))
        .map_err(|why| Box::new(unwritable(path, &why)))?;
    made_confined(
        ctx,
        recording,
        stack,
        path,
        text.as_bytes(),
        Readable::Owner,
    )
}

/// The value the secret store holds for each of `plugin`'s secrets, by its id.
fn holding(ctx: &Ctx, plugin: &str) -> impl Fn(&str) -> Option<String> {
    let secrets = crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .unwrap_or_default();
    let plugin = plugin.to_owned();
    move |secret: &str| {
        secrets
            .value(&crate::settings::named(&plugin, secret))
            .map(str::to_owned)
    }
}

/// Write one file beneath `stack` that a plugin's service reads, readable as `readable`
/// says, through no link on the way to it, since the directory is one its container
/// writes.
fn made_confined(
    ctx: &Ctx,
    recording: (&str, &str, &Path),
    stack: &Path,
    path: &Path,
    content: &[u8],
    readable: Readable,
) -> Result<(), Box<Problem>> {
    recorded_making(ctx, recording, path, false)?;
    ctx.seams
        .confined
        .written(path, stack, content, readable)
        .map_err(|fault| Box::new(unwritable(path, &fault.message)))
}
