//! A household member asking for a code to sign in by, at a household whose accounts an
//! outside service holds.

use std::sync::Arc;

use lemonfiber_api::admission::coded::CODE;
use lemonfiber_core::ports::service::Household;
use lemonfiber_fixtures::household::Recognising;

use crate::surface::Surface;
use crate::world::World;
use crate::Seen;

/// A code asked for, and what it answered kept.
pub(crate) async fn every(seen: &mut Seen) -> Option<()> {
    let household: Arc<dyn Household> = Recognising::by_code("a7f3");
    let surface =
        Surface::over_household(World::named("sign-in-code"), Arc::new(household)).await?;
    seen.answered(
        surface
            .asked("POST", CODE, Some(&serde_json::json!({})))
            .await
            .envelopes,
    );
    Some(())
}
