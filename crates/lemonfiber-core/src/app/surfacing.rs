//! What the web surface asks of the core for itself, where each goes, and what each
//! answers under the kind the contract publishes for it.

use serde::Serialize;

use super::standing::StackStanding;
use super::{routed, standing, Ctx, Outcome, Surfacing};
use crate::error::Problem;
use crate::model::kind::{self, Kind};
use crate::model::Envelope;
use crate::notify::events::Heard;
use crate::screening::{Renewed, Untuned};

/// What the web surface asked of the core for itself answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// What the web API composes its routes and its answers from.
    Standing(StackStanding),
    /// Which service told of an event it posted.
    Heard(Heard),
    /// The grant to play of a member whose client spoke.
    Renewed(Renewed),
    /// How many live streams whose passes lapsed were closed.
    Untuned(Untuned),
}

impl Answer {
    /// The kind this answer is published under.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Standing(_) => kind::STANDING,
            Self::Heard(_) => kind::NOTIFY_HEARD,
            Self::Renewed(_) => kind::RENEWED,
            Self::Untuned(_) => kind::UNTUNED,
        }
    }

    /// The shape of each answer, under its kind.
    pub fn schemas(each: &mut impl FnMut(Kind, schemars::Schema)) {
        each(
            kind::STANDING,
            schemars::schema_for!(Envelope<StackStanding>),
        );
        each(kind::NOTIFY_HEARD, schemars::schema_for!(Envelope<Heard>));
        each(kind::RENEWED, schemars::schema_for!(Envelope<Renewed>));
        each(kind::UNTUNED, schemars::schema_for!(Envelope<Untuned>));
    }
}

impl Serialize for Answer {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Standing(standing) => standing.serialize(serializer),
            Self::Heard(heard) => heard.serialize(serializer),
            Self::Renewed(renewed) => renewed.serialize(serializer),
            Self::Untuned(untuned) => untuned.serialize(serializer),
        }
    }
}

/// What the web surface asked for itself comes to.
///
/// # Errors
///
/// Whatever the read a stream polls answers with, and the refusal of an event whose key
/// no service was handed.
pub(super) async fn surfaced(ctx: &Ctx, asked: Surfacing) -> Result<Outcome, Box<Problem>> {
    let answer = match asked {
        Surfacing::Polled(polled) => return Box::pin(routed(polled.twin(), ctx)).await,
        Surfacing::Standing => Answer::Standing(standing::standing(ctx)),
        Surfacing::Told(telling) => {
            Answer::Heard(crate::notify::events::heard_of(ctx, telling).await?)
        }
        Surfacing::Spoke { member } => Answer::Renewed(crate::screening::spoke(ctx, &member)),
        Surfacing::Untune => Answer::Untuned(crate::screening::untuned(ctx).await),
    };
    Ok(Outcome::Surfaced(answer))
}

#[cfg(test)]
mod tests;
