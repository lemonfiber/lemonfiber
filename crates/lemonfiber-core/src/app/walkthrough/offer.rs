//! Whether to offer a walkthrough at all, and which one.
//!
//! Asked before anything is promised. A stack that acquires nothing gets the walk that
//! suits it, and a stack that cannot search gets told what is missing — because offering
//! a walk that must stop at its first step is the product demonstrating that it does not
//! know what it is doing, on the operator's very first minute with it.

use super::super::targets::OpenArr;
use super::super::Ctx;
use crate::ports::service::Catalogue;
use crate::walkthrough::Why;

/// What this stack should be offered.
pub(super) async fn offered(ctx: &Ctx, arrs: &[OpenArr]) -> Why {
    let protocols = ctx.settings.protocols;
    Why::of(
        protocols.usenet || protocols.torrent,
        has_indexers(arrs).await,
    )
}

/// Whether anything is configured to search with.
///
/// Any one service having an indexer is enough: they are wired to a shared indexer
/// manager, so a stack where one \*arr answers and another has not finished starting has
/// indexers — reading the silent one as zero would withdraw an offer the stack can honour.
///
/// A service that cannot be asked contributes nothing rather than a zero, for the same
/// reason: it is the difference between "there are none" and "I could not tell".
async fn has_indexers(arrs: &[OpenArr]) -> bool {
    for arr in arrs {
        if arr.service.indexer_count().await.unwrap_or(0) > 0 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::walkthrough::{Reason, Shape, Why};

    #[test]
    fn what_a_stack_is_offered_follows_from_what_it_can_do() {
        // The offer is the pure decision; what is asked of the services to reach it is
        // two booleans, and the mapping between them is proven where it is written.
        assert_eq!(Why::of(true, true), Why::Offer(Shape::Pipeline));
        assert_eq!(Why::of(false, false), Why::Offer(Shape::LibraryOnly));
        assert_eq!(Why::of(true, false), Why::Not(Reason::NoIndexers));
    }
}
