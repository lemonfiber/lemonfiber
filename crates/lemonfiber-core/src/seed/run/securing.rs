//! The egress guard, asked to hold its control server to the credential its adapter was
//! handed, and started again where it takes that credential up only once it restarts.

use std::time::SystemTime;

use lemonfiber_contract::capabilities::network::egress;

use super::enveloping::POLL;
use super::restarting::Restarting;
use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::ports::service::Tunnelling as _;
use crate::seed::State;
use crate::wiring::{Filler, Fillers};

/// What a guard that did not take its control credential up is said with.
const UNTAKEN: &str = "the egress guard did not take up the control credential it was handed";

/// Why a guard was not asked while it does not answer over its contract.
const UNANSWERED: &str = "it does not answer over its contract";

/// What asking the guard filling `network.egress-guard` among `fillers` to hold its
/// control credential came to; nothing where it held it at once, none fills it over its
/// contract, or on a rehearsal.
pub(crate) async fn secured(ctx: &Ctx, fillers: &Fillers) -> Option<Secured> {
    let (guarding, _) = fillers.filling(egress::CAPABILITY)?;
    through(ctx, fillers, guarding, ctx.seams.clock.now()).await
}

/// [`secured`] for the guard among `installed`'s services, the plugin taken as installed
/// beside every other the register holds, asked again while it does not answer over its
/// contract until `until`; nothing where none of its services fills
/// `network.egress-guard` or the stack cannot be read.
pub(crate) async fn securing_installed(
    ctx: &Ctx,
    installed: &crate::plugin::Installed,
    until: SystemTime,
) -> Option<Secured> {
    let fillers = super::installed_beside(ctx, installed)?;
    Box::pin(secured_by(ctx, &fillers, &installed.plugin, until)).await
}

/// [`secured`] for the guard among the services `plugin` brings, asked again while it
/// does not answer over its contract until `until`; nothing where none of them fills
/// `network.egress-guard`.
pub(crate) async fn secured_by(
    ctx: &Ctx,
    fillers: &Fillers,
    plugin: &str,
    until: SystemTime,
) -> Option<Secured> {
    let guarding = fillers.services().find(|one| {
        one.brought_by() == Some(plugin)
            && one.provides.iter().any(|name| name == egress::CAPABILITY)
    })?;
    through(ctx, fillers, guarding, until).await
}

/// What a rehearsal of installing or updating `installed` says of the guard among its
/// services speaking its contract: that its upstream may restart to take up the control
/// credential it is handed. Nothing where none is, or the stack cannot be read.
pub(crate) fn would_restart(ctx: &Ctx, installed: &crate::plugin::Installed) -> Option<String> {
    let fillers = super::installed_beside(ctx, installed)?;
    let guarding = fillers.services().find(|one| {
        one.brought_by() == Some(installed.plugin.as_str())
            && one.contracted(egress::CAPABILITY, egress::MAJOR)
    })?;
    let upstream = fillers.fronted_by(guarding).unwrap_or(guarding);
    Some(format!(
        "{} may restart to take up the control credential it is handed",
        upstream.id
    ))
}

/// Ask `guarding` to hold its control credential, asked again while it does not answer
/// over its contract until `until`.
async fn through(
    ctx: &Ctx,
    fillers: &Fillers,
    guarding: &Filler,
    until: SystemTime,
) -> Option<Secured> {
    if ctx.dry_run {
        return None;
    }
    loop {
        match spoken(ctx, guarding, egress::CAPABILITY, egress::MAJOR).await {
            Spoken::Over(adapter) => {
                let restarting = Restarting::of(fillers, guarding, adapter.clone());
                let asked = egress::Adapter(adapter).secure().await;
                return secured_as(restarting.taken_up(ctx, asked).await);
            }
            Spoken::Not => return None,
            Spoken::Unanswered if ctx.seams.clock.now() >= until => {
                return Some(Secured::Untaken {
                    why: UNANSWERED.to_owned(),
                });
            }
            Spoken::Unanswered => tokio::time::sleep(POLL).await,
        }
    }
}

/// What asking a guard to hold its control credential came to, where it was more than
/// held at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Secured {
    /// The guard's upstream restarted to take it up.
    Restarted {
        /// The service restarted.
        service: String,
    },
    /// The guard did not take it up.
    Untaken {
        /// Why.
        why: String,
    },
}

impl Secured {
    /// The sentence the operator is told.
    pub(crate) fn said(&self) -> String {
        match self {
            Self::Restarted { service } => {
                format!("{service} restarted to take up the control credential it was handed")
            }
            Self::Untaken { why } => format!("{UNTAKEN}: {why}"),
        }
    }
}

/// What taking up `state` came to, where it was more than held at once.
fn secured_as(state: State) -> Option<Secured> {
    match state {
        State::Restarted { service } => Some(Secured::Restarted { service }),
        State::Failed { detail: why }
        | State::Skipped { reason: why }
        | State::Unmatched { reason: why } => Some(Secured::Untaken { why }),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
