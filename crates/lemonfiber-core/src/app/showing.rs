//! The household's dashboard, as the core applies it the panel list over `dashboard.show`.

use std::sync::Arc;

use lemonfiber_contract::capabilities::dashboard::show;

use crate::app::targets::{spoken, Spoken};
use crate::app::Ctx;
use crate::ports::service::Showing;
use crate::wiring::Fillers;

pub(crate) mod panels;

/// The dashboard as the core asks it, over `dashboard.show`: nothing where nothing fills
/// the capability, or its filler speaks no contract or cannot be asked over it.
pub(crate) async fn asked(ctx: &Ctx, fillers: &Fillers) -> Option<Arc<dyn Showing>> {
    let (filler, _) = fillers.filling(show::CAPABILITY)?;
    match spoken(ctx, filler, show::CAPABILITY, show::MAJOR).await {
        Spoken::Over(adapter) => Some(Arc::new(show::Adapter(adapter))),
        Spoken::Unanswered | Spoken::Not => None,
    }
}
