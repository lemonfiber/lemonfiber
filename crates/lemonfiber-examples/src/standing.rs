//! What the web API composes its routes and its answers from, read as it reads it.

use lemonfiber_core::app::{dispatch, Command, Surfacing};

use crate::world::World;
use crate::Seen;

/// What the web API composes its routes and its answers from, kept.
pub(crate) async fn every(seen: &mut Seen) -> Option<()> {
    let world = World::named("standing");
    let answered = dispatch(Command::Surface(Surfacing::Standing), &world.ctx).await;
    seen.answered(
        answered
            .ok()
            .and_then(|outcome| world.read(outcome.envelope().to_json())),
    );
    Some(())
}
