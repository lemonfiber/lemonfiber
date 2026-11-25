//! What this surface composes its routes and its answers from, read through the dispatcher.

use lemonfiber_core::app::standing::StackStanding;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, Surfacing};
use lemonfiber_core::error::Problem;

/// What this surface composes its routes and its answers from, as `ctx`'s machine stands:
/// nothing standing where the dispatcher answers with anything else.
pub(crate) async fn standing(ctx: &Ctx) -> StackStanding {
    stood(dispatch(Command::Surface(Surfacing::Standing), ctx).await)
}

/// What a standing read answered, and nothing standing where it answered anything else.
fn stood(answered: Result<Outcome, Box<Problem>>) -> StackStanding {
    match answered {
        Ok(Outcome::Surfaced(lemonfiber_core::app::surfacing::Answer::Standing(report))) => report,
        _ => StackStanding::default(),
    }
}

#[cfg(test)]
mod tests;
