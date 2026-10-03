//! The household a member signs in against, as the stack holds it at that moment.
//!
//! The web surface checks a member's name and password with the media server, and
//! asks it again on every call whether that member's account still stands. Both
//! questions go to the household the stack holds when they are asked: a stack seeded
//! after the surface started, or one whose admin password was minted again, is
//! answered by what it holds now rather than by what it held at start.

use std::sync::Arc;

use super::targets::jellyfin_reader;
use super::Ctx;
use crate::ports::service::Household;

/// The media server's household, opened from what the stack holds now.
///
/// Nothing where the stack's manifest cannot be read, where it runs no media server,
/// or where no admin password is recorded for it yet. Each of those is a stack with
/// no household to ask, and a member is refused rather than admitted on it.
#[must_use]
pub fn household(ctx: &Ctx) -> Option<Arc<dyn Household>> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let server = jellyfin_reader(ctx, &manifest.services)?;
    Some(Arc::new(server))
}

#[cfg(test)]
mod tests;
