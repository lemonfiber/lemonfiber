//! The stream's source in a run: the gather the dashboard already runs on.
//!
//! Not a gather written for the web. The terminal screen and the browser read
//! the same figures from the same call, so the two cannot grade the same stack
//! differently — which is the whole reason this is a source rather than a second
//! assembly of the same panels.
//!
//! What the last gather said is kept, because that is what the gather wants: a
//! source that answered a moment ago and did not this time has its figure
//! carried forward marked stale rather than blanked, and that carrying is done
//! by handing the previous snapshot back in.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::app::dashboard::gather;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::dashboard::Snapshot;
use lemonfiber_core::model::{kind, Envelope};
use tokio::sync::Mutex;

use super::live::Gathers;
use super::wire::{Nature, Rendered};

/// The dashboard's gather, as the stream's source.
pub struct Dashboard {
    /// What the gather reaches the outside world through.
    ctx: Arc<Ctx>,
    /// The snapshot the next gather replaces, where there has been one.
    last: Mutex<Option<Snapshot>>,
}

impl Dashboard {
    /// The dashboard gathered against this context.
    #[must_use]
    pub fn against(ctx: Arc<Ctx>) -> Self {
        Self {
            ctx,
            last: Mutex::new(None),
        }
    }
}

#[async_trait]
impl Gathers for Dashboard {
    async fn gather(&self) -> Option<Rendered> {
        let mut last = self.last.lock().await;
        let snapshot = gather(&self.ctx, last.as_ref()).await;
        let rendered = Rendered::of(Nature::State, &Envelope::new(kind::DASHBOARD, &snapshot));
        *last = Some(snapshot);
        rendered
    }
}
