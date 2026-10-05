//! What is installed and what the stack wires to what, as the stream says them.
//!
//! Each is said as the envelope its read answers with, to a listener as it arrives and
//! to everyone when it changes, and not on every tick: a phone redraws its plugin
//! screen and its wiring from these, and an unchanged envelope is nothing to wake it
//! for. An install that leaves a capability contested changes both.
//!
//! **Nothing is asked of a source on a tick.** Whether what is installed changed is
//! read off the record alone; whether each source still answers is asked only when the
//! envelope is said, which is when a listener arrives or the record has moved.
//!
//! **What cannot be read is not said.** A record or a wiring that cannot be read is
//! left unsaid rather than said as nothing installed or nothing wired, and a client
//! asking the read is answered with the refusal.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::app::{plugins, Ctx, Outcome};
use tokio::sync::Mutex;

use super::live::Gathers;
use super::wire::{Nature, Rendered};

/// The plugins and the wiring, as the stream's source.
pub struct Extending {
    /// What the readings reach the machine through.
    ctx: Arc<Ctx>,
    /// The record as last said, so the plugins are said again only on a change.
    installed: Mutex<Option<String>>,
    /// The wiring as last said, for the same reason.
    wired: Mutex<Option<Rendered>>,
}

impl Extending {
    /// The plugins and the wiring read against this context.
    #[must_use]
    pub fn against(ctx: Arc<Ctx>) -> Self {
        Self {
            ctx,
            installed: Mutex::new(None),
            wired: Mutex::new(None),
        }
    }
}

#[async_trait]
impl Gathers for Extending {
    async fn gather(&self, joined: bool) -> Vec<Rendered> {
        self.said(joined).await
    }
}

impl Extending {
    /// What is owed now: what is installed, then what it is wired to.
    async fn said(&self, joined: bool) -> Vec<Rendered> {
        let mut said = Vec::new();
        said.extend(self.plugins(joined).await);
        said.extend(self.wiring(joined).await);
        said
    }

    /// What is installed, where it is owed and can be read.
    async fn plugins(&self, joined: bool) -> Option<Rendered> {
        let recorded = serde_json::to_string(&plugins::recorded(&self.ctx).ok()?).ok()?;
        let mut told = self.installed.lock().await;
        if !joined && told.as_deref() == Some(recorded.as_str()) {
            return None;
        }
        let listed = plugins::installed(&self.ctx).await.ok()?;
        *told = Some(recorded);
        Rendered::of(Nature::State, &Outcome::Plugins(listed).envelope())
    }

    /// What the stack wires to what, where it is owed and can be read.
    async fn wiring(&self, joined: bool) -> Option<Rendered> {
        let report = lemonfiber_core::wiring::listing(&self.ctx).ok()?;
        let rendered = Rendered::of(Nature::State, &Outcome::Wiring(report).envelope())?;
        let mut told = self.wired.lock().await;
        if !joined && told.as_ref() == Some(&rendered) {
            return None;
        }
        *told = Some(rendered.clone());
        Some(rendered)
    }
}

/// Several sources, gathered in turn as one.
///
/// The stream has one gather, so what it says from more than one source is said from
/// one that asks each in the order given.
pub struct Together(pub Vec<Arc<dyn Gathers>>);

impl Together {
    /// What the stream says in a run: the dashboard first, then what is installed and
    /// what it is wired to.
    #[must_use]
    pub fn in_a_run(ctx: &Arc<Ctx>) -> Self {
        Self(vec![
            Arc::new(super::dashboard::Dashboard::against(Arc::clone(ctx))),
            Arc::new(Extending::against(Arc::clone(ctx))),
        ])
    }
}

#[async_trait]
impl Gathers for Together {
    async fn gather(&self, joined: bool) -> Vec<Rendered> {
        each(&self.0, joined).await
    }
}

/// What each source says, in the order given.
async fn each(sources: &[Arc<dyn Gathers>], joined: bool) -> Vec<Rendered> {
    let mut said = Vec::new();
    for source in sources {
        said.extend(source.gather(joined).await);
    }
    said
}
