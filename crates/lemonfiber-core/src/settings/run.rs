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
        Settling::Set {
            setting,
            value,
            agreement,
        } => Asked {
            setting,
            value: Some(value),
            agreement: agreement.as_deref(),
        },
        Settling::Clear { setting, agreement } => Asked {
            setting,
            value: None,
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
    let crossing = Holding::Crossing(setting.to_owned());
    let reading: Vec<(&Installed, String)> = resolved.reading(|reading| {
        installed
            .iter()
            .flat_map(|one| {
                one.services
                    .iter()
                    .filter(|placed| {
                        crate::plugin::variables(one, placed, reading)
                            .iter()
                            .any(|variable| variable.holding == crossing)
                    })
                    .map(move |placed| (one, placed.service.clone()))
            })
            .collect()
    });
    if reading.is_empty() {
        return Vec::new();
    }
    let mut touched: Vec<&Installed> = reading.iter().map(|(one, _)| *one).collect();
    touched.dedup_by(|one, other| one.plugin == other.plugin);
    let services: Vec<String> = reading.into_iter().map(|(_, service)| service).collect();
    let unwritten = super::files::written_by(ctx, &touched, resolved)
        .err()
        .map(|why| why.to_string());
    let running = restarting::running(ctx, &services).await;
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
