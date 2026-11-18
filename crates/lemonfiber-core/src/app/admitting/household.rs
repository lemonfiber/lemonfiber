//! Where the household is found at the moment somebody is checked against it.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::time::Instant;

use crate::app::Ctx;
use crate::ports::service::Household;

/// How long a household opened once is asked again before it is opened afresh.
pub const KEPT_FOR: Duration = Duration::from_secs(10);

/// Where the household is found, at the moment somebody is checked against it.
#[async_trait::async_trait]
pub trait AtHand: Send + Sync {
    /// The household as it stands now, or nothing where there is none to ask.
    async fn now(&self, ctx: &Ctx) -> Option<Arc<dyn Household>>;

    /// Whether the household vouches for whoever holds account `id` now, having just
    /// proved its password.
    fn vouches_for(&self, ctx: &Ctx, id: &str) -> bool;

    /// Whether `token` claims the invitation standing on account `id` now.
    async fn offers_claim(&self, _ctx: &Ctx, _id: &str, _token: &str) -> bool {
        false
    }

    /// Spend the claim on account `id`'s invitation, where there is a record to spend it
    /// from.
    fn claim_spent(&self, _ctx: &Ctx, _id: &str) {}
}

/// One household, the same at every asking, vouching for everybody it signs in.
#[async_trait::async_trait]
impl AtHand for Arc<dyn Household> {
    async fn now(&self, _: &Ctx) -> Option<Arc<dyn Household>> {
        Some(Arc::clone(self))
    }

    fn vouches_for(&self, _: &Ctx, _: &str) -> bool {
        true
    }
}

/// The household of the stack the context runs, opened from it at each asking.
pub struct Stacked;

#[async_trait::async_trait]
impl AtHand for Stacked {
    async fn now(&self, ctx: &Ctx) -> Option<Arc<dyn Household>> {
        crate::app::members::household(ctx).await
    }

    fn vouches_for(&self, ctx: &Ctx, id: &str) -> bool {
        crate::app::members::vouched_for(ctx, id)
    }

    async fn offers_claim(&self, ctx: &Ctx, id: &str, token: &str) -> bool {
        crate::app::members::offers_claim(ctx, id, token).await
    }

    fn claim_spent(&self, ctx: &Ctx, id: &str) {
        crate::app::members::claim_spent(ctx, id);
    }
}

/// A household opened from another and kept for [`KEPT_FOR`].
pub struct Remembered {
    asked: Arc<dyn AtHand>,
    held: Mutex<Option<(Instant, Arc<dyn Household>)>>,
}

impl Remembered {
    /// The household `asked` opens, kept for a few seconds at a time.
    #[must_use]
    pub fn over(asked: Arc<dyn AtHand>) -> Self {
        Self {
            asked,
            held: Mutex::new(None),
        }
    }

    fn kept(&self, asked: Instant) -> Option<Arc<dyn Household>> {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.as_ref()
            .filter(|(until, _)| asked < *until)
            .map(|(_, household)| Arc::clone(household))
    }
}

#[async_trait::async_trait]
impl AtHand for Remembered {
    async fn now(&self, ctx: &Ctx) -> Option<Arc<dyn Household>> {
        let asked = Instant::now();
        if let Some(kept) = self.kept(asked) {
            return Some(kept);
        }
        let opened = self.asked.now(ctx).await;
        *self.held.lock().unwrap_or_else(PoisonError::into_inner) = opened
            .as_ref()
            .map(|household| (asked + KEPT_FOR, Arc::clone(household)));
        opened
    }

    fn vouches_for(&self, ctx: &Ctx, id: &str) -> bool {
        self.asked.vouches_for(ctx, id)
    }

    async fn offers_claim(&self, ctx: &Ctx, id: &str, token: &str) -> bool {
        self.asked.offers_claim(ctx, id, token).await
    }

    fn claim_spent(&self, ctx: &Ctx, id: &str) {
        self.asked.claim_spent(ctx, id);
    }
}

/// The household `ctx` holds now, opened on a thread made for blocking.
pub(super) async fn opened(ctx: &Ctx) -> Option<Arc<dyn Household>> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(ctx.admitting.household.now(&ctx))
    })
    .await
    .ok()
    .flatten()
}

/// Whether the household vouches for account `id`, asked on a thread made for blocking.
pub(super) async fn vouched(ctx: &Ctx, id: &str) -> bool {
    let (ctx, id) = (ctx.clone(), id.to_owned());
    tokio::task::spawn_blocking(move || ctx.admitting.household.vouches_for(&ctx, &id))
        .await
        .unwrap_or(false)
}

/// Whether `token` claims account `id`'s invitation, asked on a thread made for blocking.
pub(super) async fn claimable(ctx: &Ctx, id: &str, token: &str) -> bool {
    let (ctx, id, token) = (ctx.clone(), id.to_owned(), token.to_owned());
    tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current()
            .block_on(ctx.admitting.household.offers_claim(&ctx, &id, &token))
    })
    .await
    .unwrap_or(false)
}

/// The display name of account `id`, where the household holds it.
pub(super) async fn named(ctx: &Ctx, id: &str) -> Option<String> {
    let household = opened(ctx).await?;
    let members = household.household().await.ok()?;
    members
        .into_iter()
        .find(|member| member.id == id)
        .map(|member| member.name)
}
