//! The secrets lemonfiber makes for a plugin's own services at install.
//!
//! Made before anything is written, because the plugin's secrets files, starting files
//! and adapters' `upstream.json` are written from the store. One already held is kept,
//! so a reinstall or an update hands the upstream the credential it already answers to,
//! and a part moved in place is filed the credential the stack already holds for it.

use lemonfiber_contract::adapter::{UPSTREAM_PASSWORD, UPSTREAM_USERNAME};
use lemonfiber_manifest::container::{CarriedFrom, Secret};
use lemonfiber_manifest::ApiKind;
use lemonfiber_plugin::Manifest;

use super::super::Ctx;
use super::following::diagnosed;
use crate::error::Problem;
use crate::journal::{Change, Kind};
use crate::plugin::first_party::FirstParty;
use crate::plugin::Installed;

/// Hold a freshly minted value for each secret `manifest` mints that `would` holds none
/// of yet, journalling each by its fingerprint; for a plugin `trusted` names, the
/// credential the stack already holds for a part it moves in place of instead.
///
/// # Errors
///
/// Where there is no store to hold them in, the machine supplies no random bytes, or the
/// store or the journal would not take them.
pub(super) fn minted(
    ctx: &Ctx,
    would: &Installed,
    manifest: &Manifest,
    stamp: &str,
    trusted: &[FirstParty],
) -> Result<(), Box<Problem>> {
    if !manifest.secrets.iter().any(|secret| secret.minted.is()) {
        return Ok(());
    }
    let plugin = would.plugin.as_str();
    let first_party = crate::plugin::first_party::holds(trusted, would);
    let held = crate::settings::secrets_at(ctx)
        .zip(crate::app::targets::layout(ctx))
        .ok_or(crate::config::store::Failure::Nowhere);
    let (secrets, paths) = held.as_ref().map_err(diagnosed)?;
    let minting: Vec<(String, String)> = manifest
        .secrets
        .iter()
        .filter(|secret| secret.minted.is())
        .map(|secret| {
            first_party
                .then(|| moved(ctx, manifest, secret))
                .flatten()
                .or_else(|| {
                    crate::plugin::seeding::minted_as(
                        ctx.seams.random.as_ref(),
                        secret.minted,
                        plugin,
                    )
                })
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
    crate::settings::filed(secrets, &keeping, ctx.seams.random.as_ref(), &|absent| {
        let changes: Vec<Change> = absent
            .iter()
            .map(|(one, current)| Change {
                at: stamp.to_owned(),
                operation: crate::plugin::owner(plugin),
                target: one.name.to_owned(),
                kind: Kind::Secret {
                    name: one.name.to_owned(),
                    from: one.from.as_str().to_owned(),
                    previous: None,
                    current: current.clone(),
                    filed: None,
                },
            })
            .collect();
        crate::app::recover::journalled(&paths.journal(), &changes)
    })
    .map_err(|failure| diagnosed(&failure))
}

/// What the stack's own service the service `secret` is of moves in place of already
/// answers to, for the part of an adapter's upstream credential `secret` holds, where
/// that service moves in place of one.
///
/// A part moved in place keeps its configuration, so its upstream answers to the
/// credential it had; one minted afresh would be one nothing set on it. A username or a
/// password is the account lemonfiber set up on the stack's service, by the API the
/// plugin's service names as its native. Anything else is the key the service wrote
/// itself, read from where the secret's `carried_from` says, and where that cannot be
/// read, the key lemonfiber recorded for it. Asked only of a first-party plugin: the
/// stack's own credential goes to nothing else.
fn moved(ctx: &Ctx, manifest: &Manifest, secret: &Secret) -> Option<String> {
    let service = manifest
        .services
        .iter()
        .find(|service| service.id == secret.of)?;
    let reserved = service.replaces.as_deref()?;
    let account = || {
        service
            .native
            .as_deref()
            .and_then(ApiKind::named)
            .and_then(crate::credential::account)
    };
    if !takes_the_place_of(ctx, manifest, service, reserved) {
        return None;
    }
    let recorded = |holds: &str| crate::app::targets::recorded_credential(ctx, reserved, holds);
    match held_as(manifest, secret) {
        Some(UPSTREAM_USERNAME) => account().map(|account| account.username.to_owned()),
        Some(UPSTREAM_PASSWORD) => account().and_then(|account| recorded(account.holds)),
        _ => secret
            .carried_from
            .as_ref()
            .and_then(|from| carried(ctx, service, reserved, from))
            .or_else(|| recorded(crate::config::API_KEY_SUFFIX)),
    }
}

/// Whether `service`, with the adapter of `manifest` in front of it, provides what the
/// stack's own service `reserved` provides first, which is the capability the
/// credentials the stack keeps for that service are named for.
fn takes_the_place_of(
    ctx: &Ctx,
    manifest: &Manifest,
    service: &lemonfiber_plugin::Service,
    reserved: &str,
) -> bool {
    let adapter = manifest
        .services
        .iter()
        .find(|one| one.fronts.as_deref() == Some(service.id.as_str()));
    let declared = |own: &[String]| match adapter {
        Some(adapter) if own.is_empty() => adapter.provides.clone(),
        _ => own.to_vec(),
    };
    let provides = declared(&service.provides);
    ctx.stack
        .checked_manifest(ctx.today())
        .ok()
        .and_then(|stack| {
            stack
                .services
                .into_iter()
                .find(|one| one.id == reserved)
                .and_then(|one| one.provides.into_iter().next())
        })
        .is_some_and(|first| provides.contains(&first))
}

/// The credential the stack's own service `reserved` wrote where `from` says, `from`'s
/// file being a path inside the container of the plugin's `service` that takes its
/// place; read only where that is a plain file beneath the directory the container owns.
fn carried(
    ctx: &Ctx,
    service: &lemonfiber_plugin::Service,
    reserved: &str,
    from: &CarriedFrom,
) -> Option<String> {
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref())?;
    let within = crate::app::targets::service_config_dir(&project, reserved);
    let path = crate::app::targets::config_file(&within, service.configuration(), &from.file)?;
    crate::credential::written::at(
        from.format,
        &from.key,
        &ctx.seams.confined.read(&path, &within).text()?,
    )
}

/// The part of an adapter's upstream credential `secret` holds, where an adapter of
/// `manifest` hands it to its upstream.
fn held_as<'a>(manifest: &'a Manifest, secret: &Secret) -> Option<&'a str> {
    manifest.services.iter().find_map(|service| {
        service
            .upstream
            .iter()
            .find(|(_, held)| held.secret == secret.id)
            .map(|(part, _)| part.as_str())
    })
}
