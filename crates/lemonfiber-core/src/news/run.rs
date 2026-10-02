//! What is new, read against the stack.

use crate::app::{conditions, Ctx};
use crate::changelog::Record;
use crate::health::Summary;

use super::News;

/// What a surface can mark as new on this stack, now.
///
/// Read from what the stack already keeps: the record this build carries, the
/// household as the request service answers for it, and the store of conditions the
/// dashboard and the doctor keep current. The checks found wrong are the ones the
/// health summary expands to, so a problem is new on one surface exactly when it is
/// wrong on another.
///
/// It cannot fail. A kind that could not be read is named as unread and the rest is
/// answered.
pub async fn news(ctx: &Ctx) -> News {
    let household = crate::household::run::household(ctx, None).await.ok();
    let known = conditions::load(ctx);
    let affected = Summary::affected(&known.all(), &ctx.stamp());
    News::of(Record::carried().as_ref(), household.as_ref(), &affected)
}

#[cfg(test)]
mod tests;
