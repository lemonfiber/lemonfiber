//! The join link an invitation carries, and the claim token it holds.
//!
//! The link is what the companion opens: the address, certificate and identifier pairing
//! material names, the name to sign in as, when it lapses, and a claim token. The token
//! goes out once, in the link, and this program keeps only its hash on the offer.

use lemonfiber_sidecar::TokenHash;

use crate::app::Ctx;
use crate::companion::Material;
use crate::error::Problem;
use crate::invitation::{Offers, HOURS_TO_CLAIM, RECORD};
use crate::model::InvitationStanding;
use crate::ports::service::Member;

/// Where the companion opens an invitation.
const JOIN: &str = "lemonfiber://join";

/// What an invitation says about the app: the link it opens, or why there is none.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Joining {
    /// The link, where one could be made.
    pub(super) join: Option<String>,
    /// Why there is no link, where there is none.
    pub(super) unjoinable: Option<String>,
}

/// The join link for `member`, offered with `standing`, or why there is none.
///
/// A claim token is minted and its hash recorded on the offer for every standing but
/// `joined`, which has nothing to claim and is handed a link that only adds the house.
pub(super) async fn joining(ctx: &Ctx, member: &Member, standing: InvitationStanding) -> Joining {
    let material = match crate::companion::paired(ctx).await {
        Ok(pairing) => pairing.material,
        Err(problem) => return unjoinable(&problem),
    };
    let ready = if standing == InvitationStanding::Joined {
        super::declining::seconds(&ctx.hours_ago(-HOURS_TO_CLAIM)).map(|expires| (expires, None))
    } else {
        claimed(ctx, &member.id)
    };
    let Some((expires, claim)) = ready else {
        return Joining {
            join: None,
            unjoinable: Some(UNREADIED.to_owned()),
        };
    };
    Joining {
        join: Some(written(&material, expires, &member.name, claim.as_deref())),
        unjoinable: None,
    }
}

/// Mint a claim token for the offer recorded on `member`, record its hash, and say when
/// the offer lapses; nothing where any step could not be taken.
fn claimed(ctx: &Ctx, member: &str) -> Option<(u64, Option<String>)> {
    let token = super::declining::minted(ctx)?;
    let offers: Offers = crate::app::record::beside(ctx, RECORD);
    let (offers, lapses) = with_claim(offers, member, &token)?;
    let expires = super::declining::seconds(&lapses)?;
    crate::app::record::keep(
        crate::app::targets::beside_env(ctx, RECORD).as_deref(),
        &offers,
    )
    .ok()?;
    Some((expires, Some(token)))
}

/// The offers with `token`'s hash on `member`'s, which must already be recorded, and when
/// that offer lapses.
fn with_claim(mut offers: Offers, member: &str, token: &str) -> Option<(Offers, String)> {
    let offer = offers.get_mut(member)?;
    offer.claim = Some(TokenHash::of(token));
    let lapses = offer.lapses.clone();
    Some((offers, lapses))
}

/// The link itself: [`JOIN`] and its parameters, in a fixed order, each percent-encoded.
#[must_use]
fn written(material: &Material, expires: u64, name: &str, claim: Option<&str>) -> String {
    let expires = expires.to_string();
    let mut said = vec![
        ("address", material.address.as_str()),
        ("fingerprint", material.fingerprint.as_str()),
        ("stack", material.stack.as_str()),
        ("expires", expires.as_str()),
        ("name", name),
    ];
    if let Some(claim) = claim {
        said.push(("claim", claim));
    }
    let query: Vec<String> = said
        .into_iter()
        .map(|(key, value)| format!("{key}={}", crate::endpoint::query_encoded(value)))
        .collect();
    format!("{JOIN}?{}", query.join("&"))
}

/// Why there is no join link, from what refused the pairing material it is built on.
fn unjoinable(problem: &Problem) -> Joining {
    let remedy = problem
        .remedies
        .first()
        .map(|remedy| format!(" {}.", remedy.action))
        .unwrap_or_default();
    Joining {
        join: None,
        unjoinable: Some(format!(
            "The app cannot be handed this invitation, because {}.{remedy}",
            problem.summary
        )),
    }
}

/// Said where the claim token could not be minted or recorded.
const UNREADIED: &str = "The app cannot be handed this invitation, because its claim could \
                         not be written down. It can still be claimed at the sign-in address.";

#[cfg(test)]
mod tests;
