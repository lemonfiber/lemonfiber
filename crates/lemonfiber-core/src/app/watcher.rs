//! The outside watcher, as the core provisions and reads it over `status.watch`.
//!
//! What it covers is worked out once, in [`covering`], and both the pass that provisions
//! the monitors and the read that answers what they see ask it, so the two never disagree
//! about what should be watched. A monitor is paired with what it covers by the record
//! [`kept`] holds of the id each provision answered, never by its name or its target.

use std::sync::Arc;

use lemonfiber_contract::capabilities::status::watch;

use crate::app::targets::{spoken, Spoken};
use crate::app::Ctx;
use crate::ports::service::Monitoring;
use crate::wiring::Fillers;

pub(crate) mod checking;
pub(crate) mod covering;
#[cfg(test)]
pub(crate) mod fixtures;
pub(crate) mod kept;
mod reading;

pub(crate) use reading::watcher;

/// The watcher as the core asks it, over `status.watch`, and the plugin filling it:
/// nothing where nothing fills the capability, or its filler speaks no contract or cannot
/// be asked over it.
pub(crate) async fn asked(ctx: &Ctx, fillers: &Fillers) -> Option<Arc<dyn Monitoring>> {
    let (filler, _) = fillers.filling(watch::CAPABILITY)?;
    match spoken(ctx, filler, watch::CAPABILITY, watch::MAJOR).await {
        Spoken::Over(adapter) => Some(Arc::new(watch::Adapter(adapter))),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// The plugin filling `status.watch`, by its id, or the service's own where the stack
/// ships it.
pub(crate) fn filled_by(fillers: &Fillers) -> Option<String> {
    let (filler, _) = fillers.filling(watch::CAPABILITY)?;
    Some(filler.brought_by().unwrap_or(&filler.id).to_owned())
}
