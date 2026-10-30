//! An upstream started again to take up what its adapter applied, journalled as a restart
//! of that service, its adapter asked nothing else until it says the upstream answers.

use lemonfiber_contract::Contracted;

use super::Ctx;
use crate::journal::{Change, Kind};
use crate::ports::service::{Applied, Failure};
use crate::seed::State;
use crate::stack::closure::Plan;
use crate::stack::compose::{build, Action};
use crate::wiring::{Filler, Fillers};

/// An adapter, and the upstream it fronts: the service started again where what it
/// applied is in force only once that service restarts.
pub(super) struct Restarting {
    adapter: Contracted,
    upstream: String,
}

impl Restarting {
    /// `adapter`, asked as `asker`, and the upstream `asker` fronts among `fillers`; the
    /// asker itself where it fronts none.
    pub(super) fn of(fillers: &Fillers, asker: &Filler, adapter: Contracted) -> Self {
        Self {
            adapter,
            upstream: fillers.fronted_by(asker).unwrap_or(asker).id.clone(),
        }
    }

    /// The adapter, to ask it what it fronts.
    pub(super) const fn adapter(&self) -> &Contracted {
        &self.adapter
    }

    /// What a rehearsal of writing `ours` over `yours` says: the write, and that the
    /// upstream may restart to take it up.
    pub(super) fn would(&self, yours: Option<String>, ours: &str) -> State {
        State::WouldWire {
            yours,
            ours: Some(format!(
                "{ours}, and {} may restart to take it up",
                self.upstream
            )),
        }
    }

    /// What `applied` comes to: written and in force, or in force once the upstream
    /// started again, said as such.
    pub(super) async fn taken_up(&self, ctx: &Ctx, applied: Result<Applied, Failure>) -> State {
        match applied {
            Ok(Applied::Now) => State::Wired,
            Ok(Applied::OnRestart) => match self.restarted(ctx).await {
                Ok(()) => State::Restarted {
                    service: self.upstream.clone(),
                },
                Err(why) => State::Failed {
                    detail: format!(
                        "{} takes it up only once it starts again, and starting it again \
                         failed: {why}",
                        self.upstream
                    ),
                },
            },
            Err(failure) => crate::seed::unreached(&failure),
        }
    }

    /// The upstream started again, journalled first, and its adapter waited on until it
    /// says the upstream answers.
    async fn restarted(&self, ctx: &Ctx) -> Result<(), String> {
        restart(ctx, &self.upstream).await?;
        let _ = crate::patience::RESTART
            .until(|| self.adapter.ready(), |ready| matches!(ready, Ok(true)))
            .await;
        Ok(())
    }
}

/// The restart of `service` written down before it is made.
fn journalled(ctx: &Ctx, service: &str) -> Result<(), String> {
    let paths = crate::app::targets::layout(ctx)
        .ok_or_else(|| "there is nowhere to journal the restart".to_owned())?;
    let change = Change {
        at: ctx.stamp(),
        operation: crate::seed::OPERATION.to_owned(),
        target: service.to_owned(),
        kind: Kind::Restarted {
            service: service.to_owned(),
        },
    };
    crate::app::recover::journalled(&paths.journal(), &[change], ctx.seams.random.as_ref())
        .map_err(|failure| format!("the restart could not be journalled: {failure}"))
}

/// Start the service `id` of the stack written to disk again, journalled first.
pub(super) async fn restart(ctx: &Ctx, id: &str) -> Result<(), String> {
    let (profile, stack) = ctx
        .stack
        .checked_manifest(ctx.today())
        .ok()
        .zip(crate::app::plugins::read(ctx).ok())
        .and_then(|(manifest, register)| {
            crate::project::profile_of(&manifest, register.installed(), id)
        })
        .zip(crate::app::project::directory(ctx))
        .ok_or_else(|| format!("{id} is not a service of the stack written to disk"))?;
    crate::app::project::unlinked(ctx).map_err(|refused| refused.summary)?;
    journalled(ctx, id)?;
    let command = build(
        &Plan::one(id, &profile),
        &ctx.settings,
        &stack,
        &Action::Restart(vec![id.to_owned()]),
        ctx.environment,
    );
    let ran = ctx
        .seams
        .runner
        .run(&command)
        .await
        .map_err(|failure| failure.to_string())?;
    if ran.succeeded() {
        Ok(())
    } else {
        Err(crate::text::plain(ran.stderr.trim()))
    }
}

#[cfg(test)]
mod tests;
