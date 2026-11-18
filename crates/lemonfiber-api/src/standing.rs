//! What this surface composes its routes and its answers from, read through the dispatcher.

use lemonfiber_core::app::standing::StackStanding;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};

/// What this surface composes its routes and its answers from, as `ctx`'s machine stands:
/// nothing standing where the dispatcher answers with anything else.
pub(crate) async fn standing(ctx: &Ctx) -> StackStanding {
    match dispatch(Command::Standing, ctx).await {
        Ok(Outcome::Standing(report)) => report,
        _ => StackStanding::default(),
    }
}
