use crate::app::{Ctx, Outcome, Settling};
use crate::credential::{Propagation, Reach};
use crate::error::{Diagnose, Problem};
use crate::model::SettingsReport;
use crate::plugin::{Holding, Installed};

use super::changing::{self, Asked};
use super::{restarting, Resolved};

pub(crate) async fn settings(ctx: &Ctx, asked: &Settling) -> Result<Outcome, Box<Problem>> {
    let asked = match asked {
        Settling::Read => return listing(ctx).map(Outcome::Settings),
        Settling::Set { values, agreement } => Asked {
            changing: values
                .iter()
                .map(|(setting, value)| (setting.as_str(), Some(value.as_str())))
                .collect(),
            agreement: agreement.as_deref(),
        },
        Settling::Clear {
            settings,
            agreement,
        } => Asked {
            changing: settings
                .iter()
                .map(|setting| (setting.as_str(), None))
                .collect(),
            agreement: agreement.as_deref(),
        },
    };
    changing::changed(ctx, &asked).await.map(Outcome::Settings)
}

/// Every setting and secret the services read, as `lemonfiber settings` lists them.
///
/// # Errors
///
/// Where the stack's manifest or the record of what is installed cannot be read.
pub fn listing(ctx: &Ctx) -> Result<SettingsReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let register = crate::app::plugins::read(ctx)?;
    Ok(super::listing::report(
        ctx,
        &manifest,
        register.installed(),
        None,
    ))
}

/// Every service held back until the operator gives what it needs to start, each with
/// what it waits for: the stack's and every installed plugin's, and each running through
/// one of them.
pub(crate) fn waiting(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    installed: &[Installed],
) -> Vec<crate::model::Waiting> {
    waiting_after(ctx, manifest, installed, &std::collections::BTreeMap::new())
}

/// This machine's services held back, read each time they are asked after.
pub(crate) struct HeldBack {
    /// Where the record of what is installed, the settings and the secrets are read.
    ctx: Ctx,
    /// The stack's manifest.
    manifest: lemonfiber_manifest::Manifest,
}

impl HeldBack {
    /// The services held back of `manifest`'s and every installed plugin's, read through
    /// `ctx`.
    pub(crate) fn of(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) -> Self {
        Self {
            ctx: ctx.clone(),
            manifest: manifest.clone(),
        }
    }
}

#[async_trait::async_trait]
impl crate::doctor::waiting::Holds for HeldBack {
    async fn waiting(&self) -> Result<Vec<crate::model::Waiting>, ()> {
        let register = crate::app::plugins::read(&self.ctx).map_err(|_| ())?;
        Ok(waiting(&self.ctx, &self.manifest, register.installed()))
    }
}

/// The services [`waiting`] holds back once `changing` is recorded.
pub(super) fn waiting_after(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    installed: &[Installed],
    changing: &std::collections::BTreeMap<&str, Option<&str>>,
) -> Vec<crate::model::Waiting> {
    let recorded = super::recorded::Recorded::of(ctx);
    let secrets = super::secrets_at(ctx).and_then(|at| super::Secrets::at(&at).ok());
    let topology = crate::project::topology(
        manifest,
        installed,
        &crate::app::targets::chosen_fillers(ctx),
        crate::app::project::pinned(ctx).is_some(),
    );
    super::held_back(
        manifest,
        installed,
        &|name| match changing.get(name) {
            Some(value) => value.map(str::to_owned),
            None => recorded.value(name).map(str::to_owned),
        },
        &|name| {
            secrets
                .as_ref()
                .is_some_and(|kept| kept.value(name).is_some())
        },
        &topology,
    )
}

pub(crate) async fn crossed(ctx: &Ctx, setting: &str) -> Vec<Propagation> {
    let Ok(register) = crate::app::plugins::read(ctx) else {
        return Vec::new();
    };
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let resolved = Resolved::of(ctx, manifest.as_ref(), register.installed());
    reached(ctx, setting, register.installed(), &resolved).await
}

pub(crate) async fn reached(
    ctx: &Ctx,
    setting: &str,
    installed: &[Installed],
    resolved: &Resolved,
) -> Vec<Propagation> {
    let (services, unwritten) = reread_by(
        ctx,
        &Holding::Crossing(setting.to_owned()),
        installed,
        resolved,
    );
    restarted_with(ctx, &services, unwritten).await
}

/// Every service whose secrets file reads `holding`, each such file written again with
/// what is held now, and why they could not be written where they could not.
pub(crate) fn reread_by(
    ctx: &Ctx,
    holding: &Holding,
    installed: &[Installed],
    resolved: &Resolved,
) -> (Vec<String>, Option<String>) {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let services: Vec<String> = super::files::every_file(manifest.as_ref(), installed, resolved)
        .into_iter()
        .filter(|(_, _, variables)| {
            variables
                .iter()
                .any(|variable| variable.holding == *holding)
        })
        .map(|(service, _, _)| service)
        .collect();
    if services.is_empty() {
        return (services, None);
    }
    let unwritten = super::files::reread(ctx, manifest.as_ref(), installed, resolved, holding)
        .err()
        .map(|why| why.to_string());
    (services, unwritten)
}

/// How far a value written into the secrets files of `services` reached each of them:
/// each running one restarted to read it, unless the files could not be written for
/// `unwritten`.
pub(crate) async fn restarted_with(
    ctx: &Ctx,
    services: &[String],
    unwritten: Option<String>,
) -> Vec<Propagation> {
    if services.is_empty() {
        return Vec::new();
    }
    let running = restarting::running(ctx, services).await;
    let unrestarted = match unwritten {
        Some(_) => None,
        None => restarting::restarted(ctx, &running).await,
    };
    services
        .iter()
        .map(|service| {
            let reach = match (&unwritten, &unrestarted, running.contains(service)) {
                (Some(why), _, _) | (None, Some(why), true) => Reach::Failed {
                    detail: why.clone(),
                },
                (None, None, true) => Reach::Updated,
                (None, _, false) => Reach::Pending {
                    detail: format!("lemonfiber up --service {service}"),
                },
            };
            Propagation {
                consumer: service.clone(),
                reach,
            }
        })
        .collect()
}
