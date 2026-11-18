//! Who the session a request carried proves it to be, asked as the web API asks it.

use lemonfiber_core::app::admitting::{Asked, Carried, Opened};
use lemonfiber_core::app::{dispatch, Command};
use lemonfiber_fixtures::ports::Chance;

use crate::world::World;
use crate::Seen;

/// The operator's session, checked as a request carrying it is, kept.
pub(crate) async fn every(seen: &mut Seen) -> Option<()> {
    let world = World::named("caller");
    let ctx = &world.ctx;
    let held = ctx
        .settings
        .admission
        .as_deref()
        .and_then(lemonfiber_core::admission::at)?;
    let session = ctx
        .admitting
        .sessions
        .opened(
            &Chance::cycling(),
            ctx.seams.clock.now(),
            Opened::Operator(held),
        )
        .await?;
    let carried = Carried {
        offered: Some(session.token),
        arrived: None,
    };
    let answered = dispatch(Command::Admission(Asked::Caller(carried)), ctx).await;
    seen.answered(
        answered
            .ok()
            .and_then(|outcome| world.read(outcome.envelope().to_json())),
    );
    Some(())
}
