//! The secrets lemonfiber makes for a plugin's own services at install.
//!
//! Made before anything is written, because the plugin's secrets files, starting files
//! and adapters' `upstream.json` are written from the store. One already held is kept,
//! so a reinstall or an update hands the upstream the credential it already answers to.

use lemonfiber_plugin::Manifest;

use super::super::Ctx;
use super::following::diagnosed;
use crate::error::Problem;
use crate::journal::{Change, Kind};

/// Hold a freshly minted value for each secret `manifest` mints that `plugin` holds none
/// of yet, journalling each by its fingerprint.
///
/// # Errors
///
/// Where there is no store to hold them in, the machine supplies no random bytes, or the
/// store or the journal would not take them.
pub(super) fn minted(
    ctx: &Ctx,
    plugin: &str,
    manifest: &Manifest,
    stamp: &str,
) -> Result<(), Box<Problem>> {
    if !manifest.secrets.iter().any(|secret| secret.minted) {
        return Ok(());
    }
    let held = crate::settings::secrets_at(ctx)
        .zip(crate::app::targets::layout(ctx))
        .ok_or(crate::config::store::Failure::Nowhere);
    let (secrets, paths) = held.as_ref().map_err(diagnosed)?;
    let minting: Vec<(String, String)> = manifest
        .secrets
        .iter()
        .filter(|secret| secret.minted)
        .map(|secret| {
            crate::plugin::seeding::mint(ctx.seams.random.as_ref())
                .map(|value| (crate::settings::named(plugin, &secret.id), value))
        })
        .collect::<Option<_>>()
        .ok_or_else(|| {
            diagnosed(&crate::config::store::Failure::NotWritten {
                path: secrets.clone(),
                reason: crate::plugin::seeding::UNMINTED.to_owned(),
            })
        })?;
    let at = ctx.seconds();
    let keeping: Vec<crate::settings::Keeping<'_>> = minting
        .iter()
        .map(|(name, value)| crate::settings::Keeping {
            name,
            value,
            from: crate::settings::Source::Minted,
            at,
        })
        .collect();
    crate::settings::filed(secrets, &keeping, &|absent| {
        let changes: Vec<Change> = absent
            .iter()
            .map(|one| Change {
                at: stamp.to_owned(),
                operation: crate::plugin::owner(plugin),
                target: one.name.to_owned(),
                kind: Kind::Secret {
                    name: one.name.to_owned(),
                    from: one.from.as_str().to_owned(),
                    previous: None,
                    current: Some(crate::credential::fingerprint(one.value)),
                    filed: None,
                },
            })
            .collect();
        crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())
    })
    .map_err(|failure| diagnosed(&failure))
}
