//! The household's dashboard, as the stack declares it and as the core applies it the
//! panel list.
//!
//! Asked over `dashboard.show` where its filler speaks the contract, and where the filler
//! is the assembled Homepage, which speaks none, told through the two files Homepage
//! reads in its configuration directory.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_contract::capabilities::dashboard::show;

use crate::app::targets::{spoken, Spoken};
use crate::app::Ctx;
use crate::origin::Origin;
use crate::ports::service::Showing;
use crate::wiring::Fillers;

mod configured;
pub(crate) mod panels;

/// How the core reaches the dashboard.
pub(crate) enum Dashboard {
    /// Asked, over its contract or through its files.
    Asked(Arc<dyn Showing>),
    /// The assembled Homepage, in an area the operator declared unmanaged, which
    /// lemonfiber writes nothing into.
    Unmanaged,
}

/// The dashboard as the core asks it: over `dashboard.show` where its filler speaks the
/// contract, otherwise the assembled Homepage told through its files under `project`.
/// Nothing where nothing fills the capability, or its filler speaks the contract and
/// cannot be asked over it.
pub(crate) async fn asked(ctx: &Ctx, fillers: &Fillers, project: &Path) -> Option<Dashboard> {
    let (filler, _) = fillers.filling(show::CAPABILITY)?;
    match spoken(ctx, filler, show::CAPABILITY, show::MAJOR).await {
        Spoken::Over(adapter) => Some(Dashboard::Asked(Arc::new(show::Adapter(adapter)))),
        Spoken::Unanswered => None,
        Spoken::Not if filler.origin != Origin::Bundled => None,
        Spoken::Not if configured::unmanaged(&ctx.settings.unmanaged) => Some(Dashboard::Unmanaged),
        Spoken::Not => Some(Dashboard::Asked(Arc::new(configured::Configured::at(
            project,
            ctx.seams.confined.clone(),
            ctx.settings
                .env_file
                .as_deref()
                .map(super::bounded::record_beside),
        )))),
    }
}
