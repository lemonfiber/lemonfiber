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
    if !manifest.secrets.iter().any(|secret| secret.minted) {
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
        .filter(|secret| secret.minted)
        .map(|secret| {
            first_party
                .then(|| moved(ctx, manifest, secret))
                .flatten()
                .or_else(|| crate::plugin::seeding::mint(ctx.seams.random.as_ref()))
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

/// What the stack's own service the service `secret` is of moves in place of already
/// answers to, for the part of an adapter's upstream credential `secret` holds, where
/// that service moves in place of one.
///
/// A part moved in place keeps its configuration, so its upstream answers to the
/// credential it had; one minted afresh would be one nothing set on it. A username or a
/// password is the account lemonfiber set up on the stack's service, by the API the stack
/// declares it answering in or, where it declares none, the one the plugin's service
/// names as its native. Anything else is the key the service wrote itself, read from
/// where the secret's `carried_from` says, then from the file the stack's declaration
/// names, and where neither can be read, the key lemonfiber recorded for it. Asked only
/// of a first-party plugin: the stack's own credential goes to nothing else.
fn moved(ctx: &Ctx, manifest: &Manifest, secret: &Secret) -> Option<String> {
    let service = manifest
        .services
        .iter()
        .find(|service| service.id == secret.of)?;
    let reserved = service.replaces.as_deref()?;
    let declared = ctx.stack.manifest().ok().and_then(|stack| {
        stack
            .services
            .into_iter()
            .find(|service| service.id == reserved)
    });
    let kind = declared
        .as_ref()
        .and_then(|service| service.api.as_ref())
        .map(|api| api.kind)
        .or_else(|| service.native.as_deref().and_then(ApiKind::named));
    let account = || kind.and_then(crate::credential::account);
    match held_as(manifest, secret) {
        Some(UPSTREAM_USERNAME) => account().map(|account| account.username.to_owned()),
        Some(UPSTREAM_PASSWORD) => account()
            .and_then(|account| crate::app::targets::recorded_secret(ctx, account.password)),
        _ => secret
            .carried_from
            .as_ref()
            .and_then(|from| carried(ctx, service, reserved, from))
            .or_else(|| declared.as_ref().and_then(|service| written(ctx, service)))
            .or_else(|| {
                crate::app::targets::recorded_secret(ctx, &crate::seed::run::published_as(reserved))
            }),
    }
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

/// The key the stack's own `service` wrote into the file its declaration names, read
/// only where that is a plain file beneath the directory its container owns.
///
/// Nothing where its declaration names no file the key is read from, or the file is not
/// there, holds no key, or leads away.
fn written(ctx: &Ctx, service: &lemonfiber_manifest::Service) -> Option<String> {
    let api = service.api.as_ref()?;
    let read = crate::credential::written::reader(api.key_source)?;
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref())?;
    let path = crate::app::targets::config_path(&project, service, api.path.as_deref())?;
    let within = crate::app::targets::service_config_dir(&project, &service.id);
    read(&ctx.seams.confined.read(&path, &within).text()?)
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
