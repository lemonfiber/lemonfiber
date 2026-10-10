//! What somebody invited with the household's defaults would be told.
//!
//! The household read answers about the accounts the media server holds. This answers
//! about nobody: the access an invitation that chose nothing grants, and what the
//! household may ask for where nobody chose otherwise for one person. It is how the
//! operator sees the member's side without reading any member's.
//!
//! **It reads no account.** Who holds one, what each has asked for and what each has
//! left are never asked, so nothing of anybody's can reach the answer. The request
//! service is asked one thing, the household's own setting, and the rest is what is
//! true of everybody in the house alike: the quality in force and whether the disk has
//! room.

use crate::app::Ctx;
use crate::asking::Policy;
use crate::error::{Diagnose, Problem};
use crate::model::{HouseholdMember, HouseholdReport, MemberAccess};
use crate::ports::service::{Asking, Headroom, Left};

use super::{allowance, handing_over, reaching};

/// What a member invited with the household's defaults would be told.
///
/// One member in the list, under no name: the defaults are nobody's, and a row is where
/// a member's side of the household is read from. The row is claimed and standing,
/// because a member is told these things once they have signed in.
pub(crate) async fn as_the_defaults(ctx: &Ctx) -> Result<HouseholdReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let mut findings = Vec::new();
    let asking = match reaching(ctx, &manifest).await {
        Ok(access) => access.requests.asking().await.ok(),
        Err(reason) => {
            findings.push(reason);
            None
        }
    };
    if asking.is_none() {
        findings.push(
            "what the household may ask for could not be read, so no policy and no \
             limit are shown — reported as unread rather than as unlimited"
                .to_owned(),
        );
    }

    let quality = crate::quality::run::recorded_selection(ctx);
    let no_room = crate::space::run::admits(ctx).await.is_err();
    let mut member = HouseholdMember {
        access: MemberAccess {
            every_library: true,
            ..MemberAccess::default()
        },
        asking: asking.map(|asking| allowance::reported(&held(asking), &[], ctx.seams.clock.now())),
        claimed: true,
        ..HouseholdMember::default()
    };
    member.to_hand_over = handing_over::to_hand_over(&member, &quality, None, no_room);

    Ok(HouseholdReport {
        rehearsed: false,
        policy: asking.as_ref().map(Policy::of),
        allows: asking
            .and_then(|asking| asking.quota)
            .map(crate::asking::limit),
        members: vec![member],
        available: true,
        findings,
        filtering: None,
    })
}

/// What the request service would hold for somebody invited under the household's own
/// setting, before they have asked for anything.
///
/// Nothing counted yet, so all of a period is left. No identifier, because there is no
/// account to write to.
fn held(asking: Asking) -> allowance::Held {
    let left = Left {
        limit: asking.quota.map(|quota| quota.requests),
        used: 0,
        days: asking.quota.map(|quota| quota.days),
    };
    allowance::Held {
        id: String::new(),
        approves_own: asking.approves_own,
        headroom: Headroom {
            films: left,
            television: left,
        },
    }
}
