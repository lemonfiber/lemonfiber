//! Starting again the upstream that takes a claim only once it restarts.

use async_trait::async_trait;

use crate::app::Ctx;
use crate::ports::service::{Failure, Ownership};
use crate::seed::run::restarting::Restarting;

/// Starting again the upstream a claimed part's adapter fronts.
#[async_trait]
pub(crate) trait Restarts: Sync {
    /// The upstream started again and its adapter waited on until it says it answers.
    ///
    /// # Errors
    ///
    /// The [`Failure`] naming the upstream, where it could not be started again.
    async fn restarted(&self) -> Result<(), Failure>;
}

/// A part as a claim reaches it: over `owner.claim`, and its upstream started again where
/// it takes a claim only once it restarts.
#[derive(Clone, Copy)]
pub(crate) struct Reaching<'a> {
    /// The part, asked over `owner.claim`.
    pub(crate) part: &'a dyn Ownership,
    /// What starts its upstream again.
    pub(crate) restarts: &'a dyn Restarts,
}

/// The upstream a seed's adapter fronts, started again as a seed starts any service again:
/// journalled first.
pub(crate) struct Restarted<'c> {
    /// The run it is started again in.
    pub(crate) ctx: &'c Ctx,
    /// The adapter and the upstream it fronts.
    pub(crate) restarting: Restarting,
}

#[async_trait]
impl Restarts for Restarted<'_> {
    async fn restarted(&self) -> Result<(), Failure> {
        self.restarting
            .restarted(self.ctx)
            .await
            .map_err(|detail| Failure::Refused {
                service: self.restarting.upstream().to_owned(),
                detail,
            })
    }
}
