//! The settings, as the operator's stream says them.
//!
//! Said as the envelope `GET /api/settings` answers with, to a listener as it arrives
//! and to everyone when it changes, and only to the operator: a key listening to the
//! same stream passes over it, as the read refuses it. A listing that cannot be read is
//! left unsaid rather than said as no settings.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::app::{Ctx, Outcome};
use tokio::sync::Mutex;

use super::live::Gathers;
use super::wire::{Nature, Rendered};

/// The settings, as the stream's source.
pub struct Settings {
    /// What the reading reaches the machine through.
    ctx: Arc<Ctx>,
    /// The settings as last said, so they are said again only on a change.
    told: Mutex<Option<Rendered>>,
}

impl Settings {
    /// The settings read against this context.
    #[must_use]
    pub fn against(ctx: Arc<Ctx>) -> Self {
        Self {
            ctx,
            told: Mutex::new(None),
        }
    }
}

#[async_trait]
impl Gathers for Settings {
    async fn gather(&self, joined: bool) -> Vec<Rendered> {
        self.said(joined).await.into_iter().collect()
    }
}

impl Settings {
    /// The settings, where they are owed and can be read.
    async fn said(&self, joined: bool) -> Option<Rendered> {
        let report = lemonfiber_core::listed_settings(&self.ctx).ok()?;
        let rendered =
            Rendered::of(Nature::State, &Outcome::Settings(report).envelope())?.to_the_operator();
        let mut told = self.told.lock().await;
        if !joined && told.as_ref() == Some(&rendered) {
            return None;
        }
        *told = Some(rendered.clone());
        Some(rendered)
    }
}
