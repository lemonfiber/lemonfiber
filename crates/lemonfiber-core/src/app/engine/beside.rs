//! What a start readies beside the project before it writes it.

use super::Ctx;

/// Why each thing a start readies beside the project was not readied, where it was not.
#[derive(Default)]
pub(super) struct Beside {
    /// Why the pass service is left out.
    pub(super) passing: Option<String>,
    /// Why the addresses of the engine's host were not asked or kept for a fence.
    pub(super) fencing: Option<String>,
    /// Why the group owning the render devices of the engine's host was not asked or kept.
    pub(super) rendering: Option<String>,
}

/// Ready the pass service and ask the engine for its host's addresses and render group
/// before a start writes the project that reads them, and say why each is not readied,
/// where it is not. A rehearsal asks nothing.
pub(super) async fn beside_readied(ctx: &Ctx) -> Beside {
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return Beside::default();
    };
    Beside {
        passing: if ctx.dry_run {
            None
        } else {
            crate::app::project::pass_readied(ctx, &manifest).await
        },
        fencing: Box::pin(crate::app::project::fence_readied(
            ctx,
            &manifest,
            lemonfiber_bundled::FENCE,
        ))
        .await,
        rendering: Box::pin(crate::app::project::render_readied(
            ctx,
            &manifest,
            lemonfiber_bundled::FENCE,
        ))
        .await,
    }
}
