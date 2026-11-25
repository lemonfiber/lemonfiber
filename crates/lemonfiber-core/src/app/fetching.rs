//! The reads answered with bytes and their media type rather than a report.

use crate::error::Problem;
use crate::model::Octets;

use super::{support, Ctx, Fetching};

pub(super) async fn fetched(ctx: &Ctx, asked: Fetching) -> Result<Octets, Box<Problem>> {
    match asked {
        Fetching::Title { member, id, which } => {
            crate::screening::picture(ctx, &member, &id, which).await
        }
        Fetching::Found { member, kind, id } => {
            crate::wishing::poster(ctx, &member, &kind, &id).await
        }
        Fetching::Cover { member, unit, id } => {
            crate::wishing::cover(ctx, &member, unit, &id).await
        }
        Fetching::Kept { member, reference } => {
            let named = Some(member.as_str()).filter(|member| !member.is_empty());
            crate::wishing::kept_cover(ctx, named, &reference).await
        }
        Fetching::Bundle { name } => support::held(ctx, &name),
    }
}
