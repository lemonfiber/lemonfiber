//! What the engine's host is asked before a project is written that holds a fenced
//! service or one taking `render`: its addresses and the group owning its render devices.

use lemonfiber_manifest::Service;

use super::super::Ctx;
use crate::plugin::Installed;

/// Why each answer the engine's host was asked for was not kept, where it was not.
#[derive(Default)]
pub(crate) struct Asked {
    /// Why its addresses were not asked or kept for a fence.
    pub(crate) fencing: Option<String>,
    /// Why the group owning its render devices was not asked or kept.
    pub(crate) rendering: Option<String>,
}

/// Ask the engine's host what starting `stack`'s and `installed`'s services reads of it,
/// and keep each answer where the project reads it.
pub(crate) async fn asked(ctx: &Ctx, stack: &[Service], installed: &[Installed]) -> Asked {
    let image = lemonfiber_bundled::FENCE;
    Asked {
        fencing: Box::pin(super::fencing::readied(ctx, stack, installed, image)).await,
        rendering: Box::pin(super::rendering::readied(ctx, stack, installed, image)).await,
    }
}
