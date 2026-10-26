//! Replacing a secret lemonfiber minted for a plugin's upstream.
//!
//! The upstream reads its credential from its own settings, which it rewrites once it runs,
//! so the new value is written where the old one stands rather than from the template: the
//! line the kept template puts the placeholder on is found by the text around it, and only
//! the value there changes. Every file is worked out before anything is held or written, so
//! a file whose line is gone refuses the replacement with the old value still in force.
//!
//! **Every copy is reached in the same run.** A secrets file anywhere on the machine that
//! reads the secret, the plugin's own or another service's it crosses to, is written again
//! and its service restarted. Where the upstream is a curator, the indexer's application,
//! the subtitle finder, a quality sync and the request gate's route each hold the key
//! too, and each is given the new one: a copy left behind is one that stops working the
//! moment the replacement lands.

use std::path::{Path, PathBuf};

use lemonfiber_manifest::Service;

use super::rotating::would_rotate;
use crate::app::Ctx;
use crate::credential::{fingerprint, Held, Propagation, Reach, Rotation, Settled};
use crate::journal::{Change, Kind};
use crate::plugin::program::Starting;
use crate::plugin::{Holding, Installed, Placed, Secret};
use crate::ports::filesystem::{Beneath, Readable};
use crate::seed::State;
use crate::wiring::{Filler, Fillers};

/// What a rehearsal says about replacing a minted secret.
const REPLACING: &str = "a real run would mint a new value, write it where the old one stands \
     in the service's settings, its secrets file and its adapter's upstream.json, and restart \
     what reads it. Nothing was replaced.";

/// The installed `plugin` that `held` is a secret lemonfiber minted for, and the secret.
pub(super) fn minted<'a>(
    held: &Held,
    plugin: &str,
    installed: &'a [Installed],
) -> Option<(&'a Installed, &'a Secret)> {
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
                .map(|secret| (owner, secret))
        })
}

/// What a minted secret is replaced amid: the stack's services and where it is written,
/// and who fills what, which say which other services hold a copy of it.
pub(crate) struct Amid<'a> {
    /// The services the stack declares.
    pub(crate) services: &'a [Service],
    /// Who fills each ask, and where each is reached.
    pub(crate) fillers: &'a Fillers,
    /// Where the stack is written.
    pub(crate) project: Option<&'a Path>,
}

/// Replace the minted secret `secret` of `installed` with a fresh value, everywhere it is
/// written and in every copy another service holds, and restart what reads it.
pub(super) async fn rotate(
    ctx: &Ctx,
    held: &Held,
    installed: &Installed,
    secret: &Secret,
    amid: &Amid<'_>,
) -> Rotation {
    if ctx.dry_run {
        return would_rotate(held, REPLACING);
    }
    replaced(ctx, &held.name, installed, secret, amid).await
}

/// Replace the secret lemonfiber minted as `filler`'s credential, on a run that means it:
/// what taking a key back from a service that held a copy of it is, as well as what a
/// rotation asked for is. Nothing where its plugin mints none or is not installed.
pub(crate) async fn reminted(ctx: &Ctx, filler: &Filler, amid: &Amid<'_>) -> Option<Rotation> {
    let (plugin, setting) = (filler.brought_by()?, filler.minted.as_deref()?);
    let register = crate::app::plugins::read(ctx).ok()?;
    let (installed, secret) = register
        .installed()
        .iter()
        .filter(|owner| owner.plugin == plugin)
        .find_map(|owner| {
            owner
                .declared
                .secrets
                .iter()
                .find(|one| one.minted && crate::settings::named(plugin, &one.id) == setting)
                .map(|one| (owner, one))
        })?;
    let name = format!("{plugin} {}", secret.id);
    Some(replaced(ctx, &name, installed, secret, amid).await)
}

/// The replacement itself, reported as `name`.
async fn replaced(
    ctx: &Ctx,
    name: &str,
    installed: &Installed,
    declared: &Secret,
    amid: &Amid<'_>,
) -> Rotation {
    let secret = declared.id.as_str();
    let refused = |detail: String| Rotation::stopped(name, Settled::Refused { detail });
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
    let setting = crate::settings::named(&installed.plugin, secret);
    let (readers, unwritten) = read_anew(ctx, installed, &setting);
    consumers.extend(
        readers
            .iter()
            .map(|service| reached(service, unwritten.clone())),
    );
    consumers.extend(handed(ctx, installed, &stack, secret));
    let mut services: Vec<String> = installed
        .services
        .iter()
        .filter(|placed| {
            hands_it(placed, secret) || files.iter().any(|(service, ..)| *service == placed.service)
        })
        .map(|placed| placed.service.clone())
        .collect();
    for reader in readers {
        if !services.contains(&reader) {
            services.push(reader);
        }
    }
    let running = crate::settings::running(ctx, &services).await;
    if let Some(why) = crate::settings::restarted(ctx, &running).await {
        consumers.push(Propagation::pending(
            "the services reading it",
            &format!("they did not restart: {why}; lemonfiber restart finishes it"),
        ));
    }
    consumers.extend(copies(ctx, &declared.of, amid).await);
    Rotation::landed(name, &fingerprint(&value), consumers)
}

/// Every secrets file on this machine reading `setting` out of the secret store, written
/// again with what it holds now: the services they are of, and why they could not be
/// written where they could not.
fn read_anew(ctx: &Ctx, installed: &Installed, setting: &str) -> (Vec<String>, Option<String>) {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let beside: Vec<Installed> = crate::app::plugins::read(ctx)
        .map(|register| register.installed().to_vec())
        .unwrap_or_default()
        .into_iter()
        .filter(|one| one.plugin != installed.plugin)
        .chain(std::iter::once(installed.clone()))
        .collect();
    let resolved = crate::settings::Resolved::of(ctx, manifest.as_ref(), &beside);
    crate::settings::run::reread_by(ctx, &Holding::Store(setting.to_owned()), &beside, &resolved)
}

/// Every other service holding a copy of the key of the service `of`, each given the new
/// one.
///
/// Only the copies this stack has: no indexer, no subtitle finder, no quality sync, or no
/// gate in front of the request service is a consumer that is not there rather than one
/// left behind.
async fn copies(ctx: &Ctx, of: &str, amid: &Amid<'_>) -> Vec<Propagation> {
    let named = amid
        .fillers
        .service(of)
        .map_or(of, |filler| filler.name.as_str());
    let held = [
        (
            crate::seed::run::resync_application(ctx, amid.fillers, of).await,
            format!("supplies {named} with indexers"),
        ),
        (
            crate::seed::run::rewatch(ctx, amid.fillers, of).await,
            format!("finds subtitles for {named}"),
        ),
        (
            crate::seed::run::reprofile(ctx, amid.fillers, of).await,
            format!("holds {named}'s quality profiles"),
        ),
    ];
    let mut copies = Vec::new();
    for (holders, what) in held {
        for (holder, state) in holders {
            copies.push(copy(format!("{holder}, which {what}"), state));
        }
    }
    if let Some(state) =
        crate::seed::run::reroute(ctx, amid.services, amid.fillers, amid.project, of).await
    {
        copies.push(copy(
            format!("the request gate, which reaches {named} for the request service"),
            state,
        ));
    }
    copies
}

/// What a copy says that ended neither holding the new key nor failing in words of its
/// own.
const UNTAKEN: &str = "it did not take the new key; lemonfiber seed says why";

/// One copy, and whether it now holds the new key.
fn copy(consumer: String, state: State) -> Propagation {
    let reach = match state {
        State::Failed { detail } => Reach::Failed {
            detail: crate::config::store::withheld_text(&detail),
        },
        settled if settled.is_settled() => Reach::Updated,
        _ => Reach::Failed {
            detail: UNTAKEN.to_owned(),
        },
    };
    Propagation { consumer, reach }
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
