//! What one member can watch, read from the media server as that member.
//!
//! The household read answers "what have we asked for". This answers "what is already
//! here", which is the other half of what somebody opens the app for and the half this
//! product could not answer at all.
//!
//! **Asked for one member, always.** There is no whole-household form of this and the
//! absence is the design: what is on the shelf is different for every account, because
//! the server applies that account's age limit, blocked kinds and library access before
//! it answers. A single answer for everybody would be a fourth copy of three rules, and
//! it would be wrong for whoever it was not read as. An operator naming a member gets
//! that member's shelf — which is also the only honest way to answer "what can my
//! child actually see".
//!
//! **The household's defaults are the one shelf that is nobody's.** It is what an
//! invitation that chose nothing grants, read about no account, so the operator can see
//! the member's side without reading any member's.

use super::targets::jellyfin_reader;
use super::{Ctx, Whom};

use crate::error::{Diagnose, Problem};
use crate::model::HeldReport;
use crate::ports::service::{Failure, Held, Household as _, Member};

/// Read what one member holds, or what the household's defaults would.
///
/// A member is matched against the id the server files them under first and against
/// their name second. A member's own session carries the id, so it is the first that
/// answers them; an operator types a name, so it is the second that answers an operator.
/// One resolution for both rather than two paths that could come to disagree about who
/// was meant.
pub(crate) async fn held(ctx: &Ctx, whose: &Whom, most: u32) -> Result<HeldReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let Some(server) = jellyfin_reader(ctx, &manifest) else {
        return Ok(unread(
            whose,
            "there is no media server to ask what the household holds, or no recorded \
             password to sign in with",
        ));
    };

    let door = crate::screening::door::standing(ctx).await;
    let Whom::Named(member) = whose else {
        return Ok(shelved(
            server.holdings(None, most).await,
            String::new(),
            String::new(),
        )
        .located(&door));
    };

    let Ok(accounts) = server.household().await else {
        return Ok(unread(
            whose,
            "the media server would not say who holds an account, so who this shelf \
             belongs to could not be established",
        ));
    };

    let Some(account) = account(&accounts, member) else {
        return Ok(unread(
            whose,
            "nobody in this household is known by that, so there is no shelf to read — \
             which is not the same as a shelf with nothing on it",
        ));
    };

    Ok(shelved(
        server.holdings(Some(&account.id), most).await,
        account.name.clone(),
        account.id.clone(),
    )
    .located(&door))
}

/// What the server answered about one shelf, said as a report about whoever it is.
///
/// `member` and `id` are empty for the household's defaults, which are nobody's.
fn shelved(answered: Result<Vec<Held>, Failure>, member: String, id: String) -> HeldReport {
    let Ok(holdings) = answered else {
        return HeldReport {
            member,
            id,
            findings: vec![
                "the media server would not say what this shelf holds, so it is reported \
                 as unread rather than as empty"
                    .to_owned(),
            ],
            ..HeldReport::default()
        };
    };
    HeldReport {
        rehearsed: false,
        member,
        id,
        holdings,
        available: true,
        findings: Vec::new(),
    }
}

/// The account a name or an id means, or nobody.
pub(crate) fn account<'a>(accounts: &'a [Member], named: &str) -> Option<&'a Member> {
    accounts.iter().find(|held| held.id == named).or_else(|| {
        accounts
            .iter()
            .find(|held| held.name.eq_ignore_ascii_case(named))
    })
}

/// A shelf that could not be read, said as that rather than as an empty one.
///
/// It names the member as they were asked for, since who they are was never
/// established, and names nobody for the household's defaults.
fn unread(whose: &Whom, why: &str) -> HeldReport {
    let member = match whose {
        Whom::Named(member) => member.clone(),
        Whom::Defaults => String::new(),
    };
    HeldReport {
        member,
        findings: vec![why.to_owned()],
        ..HeldReport::default()
    }
}

#[cfg(test)]
mod tests;
