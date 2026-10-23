//! Replacing a secret lemonfiber minted for a plugin's upstream.
//!
//! The upstream reads its credential from its own settings, which it rewrites once it runs,
//! so the new value is written where the old one stands rather than from the template: the
//! line the kept template puts the placeholder on is found by the text around it, and only
//! the value there changes. Every file is worked out before anything is held or written, so
//! a file whose line is gone refuses the replacement with the old value still in force.

use std::path::{Path, PathBuf};

use lemonfiber_manifest::container::{SecretOf, Value};

use super::rotating::would_rotate;
use crate::app::Ctx;
use crate::credential::{fingerprint, Held, Propagation, Rotation, Settled};
use crate::journal::{Change, Kind};
use crate::plugin::program::Starting;
use crate::plugin::{Installed, Placed};
use crate::ports::filesystem::{Beneath, Readable};

/// What a rehearsal says about replacing a minted secret.
const REPLACING: &str = "a real run would mint a new value, write it where the old one stands \
     in the service's settings, its secrets file and its adapter's upstream.json, and restart \
     what reads it. Nothing was replaced.";

/// The installed `plugin` that `held` is a secret lemonfiber minted for, and the secret's id.
pub(super) fn minted<'a>(
    held: &Held,
    plugin: &str,
    installed: &'a [Installed],
) -> Option<(&'a Installed, String)> {
    installed
        .iter()
        .filter(|owner| owner.plugin == plugin)
        .find_map(|owner| {
            owner
                .declared
                .secrets
                .iter()
                .find(|secret| {
                    secret.minted && held.setting == crate::settings::named(plugin, &secret.id)
                })
                .map(|secret| (owner, secret.id.clone()))
        })
}

/// Replace the minted secret `secret` of `installed` with a fresh value, everywhere it is
/// written, and restart what reads it.
pub(super) async fn rotate(
    ctx: &Ctx,
    held: &Held,
    installed: &Installed,
    secret: &str,
) -> Rotation {
    if ctx.dry_run {
        return would_rotate(held, REPLACING);
    }
    let refused = |detail: String| Rotation::stopped(&held.name, Settled::Refused { detail });
    let Some((stack, kept)) = crate::app::project::directory(ctx)
        .zip(crate::app::plugins::kept::root(ctx, &installed.plugin))
    else {
        return refused("there is no stack directory to find the service's settings in".to_owned());
    };
    let Some(value) = crate::plugin::seeding::mint(ctx.seams.random.as_ref()) else {
        return refused(crate::plugin::seeding::UNMINTED.to_owned());
    };
    let files = match rewritten(ctx, installed, &stack, &kept, secret, &value) {
        Ok(files) => files,
        Err(detail) => return refused(detail),
    };
    if let Err(detail) = held_anew(ctx, installed, secret, &value) {
        return refused(detail);
    }
    let mut consumers: Vec<Propagation> = files
        .iter()
        .map(|(service, path, bytes)| {
            let written = ctx
                .seams
                .confined
                .written(path, &stack, bytes, Readable::Owner)
                .err()
                .map(|fault| fault.message);
            reached(service, written)
        })
        .collect();
    let secrets_file = crate::settings::written(ctx, &[installed])
        .err()
        .as_ref()
        .map(ToString::to_string);
    consumers.extend(
        installed
            .services
            .iter()
            .filter(|placed| reads_it(placed, secret))
            .map(|placed| reached(&placed.service, secrets_file.clone())),
    );
    consumers.extend(handed(ctx, installed, &stack, secret));
    let services: Vec<String> = installed
        .services
        .iter()
        .filter(|placed| {
            reads_it(placed, secret)
                || hands_it(placed, secret)
                || files.iter().any(|(service, ..)| *service == placed.service)
        })
        .map(|placed| placed.service.clone())
        .collect();
    let running = crate::settings::running(ctx, &services).await;
    if let Some(why) = crate::settings::restarted(ctx, &running).await {
        consumers.push(Propagation::pending(
            "its services",
            &format!("they did not restart: {why}; lemonfiber restart finishes it"),
        ));
    }
    Rotation::landed(&held.name, &fingerprint(&value), consumers)
}

/// Each seeded starting file of `installed` kept at `kept` that names `secret`, with the
/// new value where the old one stands in `stack`, by its service and where it is; or why
/// one cannot be.
fn rewritten(
    ctx: &Ctx,
    installed: &Installed,
    stack: &Path,
    kept: &Path,
    secret: &str,
    value: &str,
) -> Result<Vec<(String, PathBuf, Vec<u8>)>, String> {
    let mut files = Vec::new();
    for placed in &installed.services {
        let templates = crate::plugin::program::shipped(Some(Starting {
            root: kept,
            service: &placed.service,
        }))?;
        for (path, template) in templates {
            let at = stack.join("config").join(&placed.service).join(&path);
            let current = match ctx.seams.confined.read(&at, stack) {
                Beneath::Read(text) => text,
                Beneath::Absent => return Err(format!("{} is not there", at.display())),
                Beneath::Escaped => {
                    return Err(format!("{} is reached through a link", at.display()))
                }
            };
            let replaced = crate::plugin::seeding::replaced(
                &template,
                current.as_bytes(),
                secret,
                value,
                ctx.seams.random.as_ref(),
            )
            .map_err(|why| format!("{}: {why}", at.display()))?;
            if let Some(bytes) = replaced {
                files.push((placed.service.clone(), at, bytes));
            }
        }
    }
    Ok(files)
}

/// Hold `value` as `installed`'s `secret`, journalled by the fingerprints of the value it
/// replaces and of itself.
fn held_anew(ctx: &Ctx, installed: &Installed, secret: &str, value: &str) -> Result<(), String> {
    let at = crate::settings::secrets_at(ctx)
        .zip(crate::app::targets::layout(ctx))
        .ok_or_else(|| crate::config::store::Failure::Nowhere.to_string())?;
    let (secrets, paths) = at;
    let name = crate::settings::named(&installed.plugin, secret);
    let stamp = ctx.stamp();
    let keeping = [crate::settings::Keeping {
        name: &name,
        value,
        from: crate::settings::Source::Minted,
        at: ctx.seconds(),
    }];
    crate::settings::kept(&secrets, &keeping, &|replaced| {
        let changes: Vec<Change> = replaced
            .iter()
            .map(|(name, previous)| Change {
                at: stamp.clone(),
                operation: crate::plugin::owner(&installed.plugin),
                target: name.clone(),
                kind: Kind::Secret {
                    name: name.clone(),
                    from: crate::settings::Source::Minted.as_str().to_owned(),
                    previous: previous.clone(),
                    current: Some(fingerprint(value)),
                    filed: None,
                },
            })
            .collect();
        crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())
    })
    .map_err(|failure| failure.to_string())
}

/// Each adapter of `installed` that hands its upstream `secret`, its `upstream.json`
/// written again with what the store holds now.
fn handed(ctx: &Ctx, installed: &Installed, stack: &Path, secret: &str) -> Vec<Propagation> {
    let secrets = crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .unwrap_or_default();
    let held = |id: &str| {
        secrets
            .value(&crate::settings::named(&installed.plugin, id))
            .map(str::to_owned)
    };
    installed
        .services
        .iter()
        .filter(|placed| hands_it(placed, secret))
        .map(|placed| {
            let at = stack
                .join("config")
                .join(&placed.service)
                .join(lemonfiber_contract::adapter::UPSTREAM_FILE);
            let written =
                crate::plugin::seeding::upstream(&placed.upstream, &held).and_then(|text| {
                    ctx.seams
                        .confined
                        .written(&at, stack, text.as_bytes(), Readable::Owner)
                        .map_err(|fault| fault.message)
                });
            reached(&placed.service, written.err())
        })
        .collect()
}

/// Whether `placed` reads `secret` out of its environment.
fn reads_it(placed: &Placed, secret: &str) -> bool {
    placed
        .fields
        .env
        .values()
        .any(|value| matches!(value, Value::Secret(SecretOf { secret: named }) if named == secret))
}

/// Whether `placed` hands its upstream `secret` in its `upstream.json`.
fn hands_it(placed: &Placed, secret: &str) -> bool {
    placed.upstream.values().any(|named| named == secret)
}

/// How far a write reached `service`: updated, or failed with why.
fn reached(service: &str, failed: Option<String>) -> Propagation {
    match failed {
        None => Propagation::updated(service),
        Some(why) => Propagation::failed(service, &why),
    }
}
