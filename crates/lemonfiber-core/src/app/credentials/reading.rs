//! Building the inventory out of what is on disk, without spending anything.
//!
//! Reads and no requests. What is recorded comes from the settings file and the secret
//! store, so an inventory costs nothing, can be taken while the stack is down, and cannot
//! itself be the thing that exhausts a provider's quota.
//!
//! That is also the limit of what it can claim, and the states say so rather than
//! rounding up. A credential is reported active where something on record says it was
//! proven against the service that takes it: the two the operator supplies at setup
//! carry a flag recording whether they were, and the ones lemonfiber mints were set on
//! their service and confirmed there before they were ever written down. Everything
//! else is stale — present, unproven, and worth a sentence rather than an alarm.

use std::path::Path;

use lemonfiber_manifest::Service;

use crate::app::targets::recorded_secret;
use crate::app::Ctx;
use crate::credential::{catalogue, fingerprint, Entry, Held, Origin, State};
use crate::plugin::Installed;

/// Where a credential lives when there is no settings file resolved to name.
const NOWHERE: &str = "nowhere yet — this machine has no settings file";

/// Every credential this stack holds, whether or not it is present.
///
/// The declared ones first, in the order they are declared, then the ones the
/// services minted for themselves in the order the manifest names them — so two runs
/// against the same stack list the same things in the same places.
pub(crate) async fn taken(
    ctx: &Ctx,
    services: &[Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
    installed: &[Installed],
) -> Vec<Held> {
    let mut taken: Vec<Held> = catalogue(ctx.settings.protocols)
        .into_iter()
        .map(|entry| recorded(ctx, entry))
        .collect();
    taken.extend(super::declining::held(ctx, services, fillers, project).await);
    taken.extend(super::gating::held(ctx, services, fillers, project).await);
    taken.extend(super::tokening::held(ctx, services, fillers, project).await);
    taken.extend(super::certifying::held(
        ctx,
        crate::seed::run::replacing::Stack {
            services,
            fillers,
            project,
        },
    ));
    let secrets = crate::settings::secrets_at(ctx);
    let kept = secrets
        .as_deref()
        .and_then(|at| crate::settings::Secrets::at(at).ok())
        .unwrap_or_default();
    let location = secrets.map(|at| at.display().to_string());
    for plugin in installed {
        taken.extend(declared(ctx, &kept, location.as_deref(), fillers, plugin));
    }
    taken
}

/// Where a plugin's secret lives while nothing holds it.
const UNHELD: &str = "nowhere yet — lemonfiber holds a plugin's secret once the plugin's \
                      recipes capture it";

/// Where a secret lemonfiber mints for a plugin lives while nothing holds it.
const UNMINTED: &str = "nowhere yet — lemonfiber mints it when the plugin is installed";

/// Every secret one installed plugin declared it would hold, attributed to it.
fn declared(
    ctx: &Ctx,
    kept: &crate::settings::Secrets,
    location: Option<&str>,
    fillers: &crate::wiring::Fillers,
    plugin: &Installed,
) -> Vec<Held> {
    plugin
        .declared
        .secrets
        .iter()
        .map(|secret| {
            let setting = crate::settings::named(&plugin.plugin, &secret.id);
            let owner = fillers
                .service(&secret.of)
                .filter(|owner| owner.brought_by() != Some(plugin.plugin.as_str()));
            let crossing = owner.is_some();
            let value = kept.value(&setting).filter(|_| !crossing);
            let present = value.is_some()
                || owner
                    .and_then(|owner| fillers.setting(owner, crate::config::API_KEY_SUFFIX))
                    .and_then(|held| recorded_secret(ctx, &held))
                    .is_some();
            Held {
                name: format!("{} {}", plugin.plugin, secret.id),
                consumers: vec![secret.of.clone()],
                location: match (crossing, value, location) {
                    (true, ..) => format!("not kept — it is {}'s, and read from there", secret.of),
                    (false, Some(_), Some(at)) => at.to_owned(),
                    _ if secret.minted => UNMINTED.to_owned(),
                    _ => UNHELD.to_owned(),
                },
                origin: if secret.minted {
                    Origin::Lemonfiber
                } else {
                    Origin::Service
                },
                from: crate::origin::Origin::Plugin {
                    named: plugin.plugin.clone(),
                },
                state: if present {
                    State::Active
                } else {
                    State::Absent
                },
                fingerprint: value.map(fingerprint),
                advisory: (!present).then(|| {
                    format!(
                        "The plugin {} declared it will hold {} for {}. Nothing holds it yet. \
                         What it said it is for: {}",
                        plugin.plugin, secret.id, secret.of, secret.why
                    )
                }),
                setting,
            }
        })
        .collect()
}

/// The settings file this machine resolved, as something to print.
fn settings_file(ctx: &Ctx) -> String {
    ctx.settings
        .env_file
        .as_deref()
        .map_or_else(|| NOWHERE.to_owned(), |path| path.display().to_string())
}

/// One declared credential, read back from where it is recorded.
fn recorded(ctx: &Ctx, entry: Entry) -> Held {
    let value = recorded_secret(ctx, entry.setting);
    let (state, advisory) = standing(ctx, &entry, value.is_some());
    Held {
        name: entry.name.to_owned(),
        setting: entry.setting.to_owned(),
        consumers: entry
            .consumers
            .iter()
            .map(|one| one.name.to_owned())
            .collect(),
        location: settings_file(ctx),
        origin: entry.origin,
        from: crate::origin::Origin::Bundled,
        state,
        fingerprint: value.as_deref().map(fingerprint),
        advisory,
    }
}

/// Where one declared credential stands, and what is worth saying about it.
fn standing(ctx: &Ctx, entry: &Entry, present: bool) -> (State, Option<String>) {
    if !present {
        return (
            State::Absent,
            Some(format!(
                "{} is not set, so anything needing it will refuse to work. It is recorded as {}.",
                entry.name, entry.setting
            )),
        );
    }
    // A credential lemonfiber minted was set on its service and confirmed there before
    // it was written down, so recording it is the proof. Merged with the origin no
    // declared entry carries, which the inventory reads a different way entirely.
    if matches!(entry.origin, Origin::Lemonfiber | Origin::Service) {
        return (State::Active, None);
    }
    match entry.proven_by.and_then(|flag| proven(ctx, flag)) {
        Some(true) => (State::Active, None),
        Some(false) => (
            State::Stale,
            Some(format!(
                "{} was kept without being proven, because that is what you chose when it was \
                 entered. It has not been checked since. Nothing here expires it.",
                entry.name
            )),
        ),
        None => (
            State::Stale,
            Some(format!(
                "{} is set and nothing on record says it has ever been proven against what \
                 takes it. Run lemonfiber doctor to find out. Nothing here expires it.",
                entry.name
            )),
        ),
    }
}

/// Whether the flag recording a proof says one happened, where the flag is set at all.
fn proven(ctx: &Ctx, flag: &str) -> Option<bool> {
    recorded_secret(ctx, flag).map(|recorded| crate::config::reads_as_on(&recorded))
}
