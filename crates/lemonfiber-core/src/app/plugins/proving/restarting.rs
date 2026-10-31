//! The upstream an installing adapter fronts, started again where it takes a claim only
//! once it restarts.

use std::path::Path;

use crate::app::Ctx;
use crate::plugin::Installed;

/// The upstream an installing adapter fronts, started again through the plugin's own
/// Compose invocation, the adapter waited on until it says the upstream answers.
pub(in crate::app::plugins) struct Restarted<'a> {
    pub(in crate::app::plugins) ctx: &'a Ctx,
    pub(in crate::app::plugins) stack: &'a Path,
    pub(in crate::app::plugins) installed: &'a Installed,
    pub(in crate::app::plugins) placed: &'a crate::plugin::Placed,
    pub(in crate::app::plugins) adapter: &'a lemonfiber_contract::Contracted,
}

#[async_trait::async_trait]
impl crate::seed::run::owning::Restarts for Restarted<'_> {
    async fn restarted(&self) -> Result<(), crate::ports::service::Failure> {
        let upstream = self
            .installed
            .upstream_of(self.placed)
            .unwrap_or(self.placed)
            .service
            .clone();
        let refused = |detail: String| crate::ports::service::Failure::Refused {
            service: upstream.clone(),
            detail,
        };
        let restart = crate::stack::compose::Action::Restart(vec![upstream.clone()]);
        let command = super::invocation(self.ctx, self.installed, self.stack, &restart);
        match self.ctx.seams.runner.run(&command).await {
            Ok(output) if output.succeeded() => {}
            Ok(refusing) => {
                return Err(refused(format!(
                    "the container engine refused to start it again: {}",
                    refusing.stderr.trim()
                )))
            }
            Err(why) => return Err(refused(why.to_string())),
        }
        crate::seed::run::answering(self.adapter).await;
        Ok(())
    }
}
