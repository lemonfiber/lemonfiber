//! Choosing what the household may ask for.
//!
//! The half of the errand that writes. What the household *has* asked for, and what each
//! member has left, are read next door in [`crate::household::run`] and answered there — and
//! this answers with that same reading, because what an operator wants to see after
//! changing a limit is the limit, on the people it applies to. A report of its own would
//! be a second description of the household able to disagree with the first.
//!
//! **Nothing named is nothing changed.** A run that gave a limit and no policy said
//! nothing about the policy, and the one in force stays. That is why the current setting
//! is read before anything is written: the two halves are one setting on the service, and
//! writing one from a value nobody chose would answer a request nobody made.
//!
//! **A per-person choice never touches the household's.** Setting somebody's own limit
//! writes against their account and leaves the default where it is, so a household that
//! trusts everybody and holds one person to five a week is one arrangement rather than
//! two settings fighting.

mod deciding;
pub(crate) mod passing_on;

pub(crate) use deciding::deciding;

use crate::asking::Policy;
use crate::error::{Diagnose, Problem};
use crate::model::HouseholdReport;
use crate::ports::service::{
    Approving as _, Asking, Headroom, Household as _, Member, Quota, Requests as _,
};

use crate::app::command::Chosen;
use crate::app::targets::{jellyfin_reader, HouseholdAccess};
use crate::app::Ctx;

/// Choose the policy, the limit, or both — for the household or for one person.
///
/// Answers with the household as it now stands, so what was chosen is read back off the
/// service rather than reported from what was sent.
pub(crate) async fn allowing(ctx: &Ctx, chosen: &Chosen) -> Result<HouseholdReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let access = reached(ctx, &manifest.services).await?;

    // Each half reads what *it* is about to change before it changes it. A per-person
    // choice read against the household's own setting would inherit the household's
    // limit onto somebody who had one of their own, which is the opposite of leaving
    // alone what nobody named.
    let said = match chosen.member.as_deref() {
        Some(name) => one_person(ctx, &access, &manifest.services, name, chosen).await?,
        None => everybody(ctx, &access, chosen).await?,
    };

    let mut report = crate::household::run::household(ctx, None).await?;
    report.findings.insert(0, said);
    Ok(report)
}

/// What is said where the service could not be asked what it holds.
const NOTHING_SET: &str = "what the household may ask for was not changed";

/// The request service, signed in, or the refusal that nothing was changed.
///
/// Its own step because both halves of this module begin with it, and because the
/// reading next door treats the same failure as a gap in a report rather than as a
/// refusal — a household that cannot be read is still worth listing, and a limit that
/// could not be written is not worth pretending was.
async fn reached(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Result<HouseholdAccess, Box<Problem>> {
    crate::household::run::reaching(ctx, services)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))
}

/// What the household is to be left holding, from what was chosen and what it holds.
///
/// Trusting everybody lifts the limit rather than leaving one counting in the
/// background: a household told nothing is limited and a service still counting is two
/// answers to one question, and the one the operator reads is the wrong one.
fn settled(chosen: &Chosen, held: &Asking) -> Result<Asking, Box<Problem>> {
    let policy = chosen.policy.unwrap_or_else(|| Policy::of(held));
    let quota = if matches!(policy, Policy::Trusted) {
        None
    } else {
        chosen.quota.or(held.quota)
    };
    if policy.needs_a_limit() && quota.is_none() {
        return Err(Box::new(crate::asking::no_limit_named()));
    }
    Ok(Asking {
        approves_own: policy.arrives_unseen(),
        quota,
    })
}

/// Write the choice against the household, so it holds for everybody nobody chose
/// otherwise for.
async fn everybody(
    ctx: &Ctx,
    access: &HouseholdAccess,
    chosen: &Chosen,
) -> Result<String, Box<Problem>> {
    let held = access
        .seerr
        .asking()
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    let wanted = settled(chosen, &held)?;
    let said = format!("the household {}", now_reads(&wanted));
    if ctx.dry_run {
        return Ok(format!("{said} — rehearsed, and nothing was written"));
    }
    access
        .seerr
        .set_asking(&wanted)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    Ok(said)
}

/// Write the choice against one person, leaving the household's own where it is.
///
/// **What is read first is theirs, not the household's.** Somebody already held to
/// three a week who is moved to waiting for approval keeps three a week; read against
/// the household's setting they would silently inherit whatever the house allows.
async fn one_person(
    ctx: &Ctx,
    access: &HouseholdAccess,
    services: &[lemonfiber_manifest::Service],
    name: &str,
    chosen: &Chosen,
) -> Result<String, Box<Problem>> {
    let account = found(ctx, services, name).await?;
    let held = access
        .seerr
        .requesting(&account.id)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    let Some(held) = held else {
        return Err(Box::new(crate::asking::never_asked_here(&account.name)));
    };
    let headroom = access
        .seerr
        .left(&held.id)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    let wanted = settled(chosen, &theirs(held.approves_own, headroom))?;
    let said = format!("{} {}", account.name, now_reads(&wanted));
    if ctx.dry_run {
        return Ok(format!("{said} — rehearsed, and nothing was written"));
    }
    // The limit first and the approval second. Between the two writes a member is held
    // to the new limit under the old policy, which is the harmless order: the other way
    // round would let requests through unseen against a limit not yet in force.
    access
        .seerr
        .set_quota(&held.id, wanted.quota)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    access
        .seerr
        .approves_own(&held.id, wanted.approves_own)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    Ok(said)
}

/// What one member is under now, from what the service says about them.
///
/// The counts come back per kind and a household chooses one figure, so the one that
/// is set is the one read back — the same reading the whole-household setting gets,
/// and for the same reason: both halves are written together.
fn theirs(approves_own: bool, headroom: Headroom) -> Asking {
    let held = |left: crate::ports::service::Left| {
        left.limit
            .zip(left.days)
            .map(|(requests, days)| Quota { requests, days })
    };
    Asking {
        approves_own,
        quota: held(headroom.films).or_else(|| held(headroom.television)),
    }
}

/// The member the name was meant for, matched the forgiving way a name is typed.
///
/// The media server is asked who is here, because that is where the household is: a
/// name matched against the request service's own list would miss everybody who has an
/// account and has never opened it.
async fn found(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    name: &str,
) -> Result<Member, Box<Problem>> {
    let Some(server) = jellyfin_reader(ctx, services) else {
        return Err(Box::new(crate::asking::unreachable(NOTHING_SET)));
    };
    let Ok(accounts) = server.household().await else {
        return Err(Box::new(crate::asking::unreachable(NOTHING_SET)));
    };
    let wanted = name.trim().to_lowercase();
    accounts
        .iter()
        .find(|account| account.name.to_lowercase().contains(&wanted))
        .cloned()
        .ok_or_else(|| {
            let there: Vec<String> = accounts.iter().map(|held| held.name.clone()).collect();
            Box::new(crate::asking::nobody_called(name, &there))
        })
}

/// What a choice comes to, as the line an operator reads it back in.
fn now_reads(wanted: &Asking) -> String {
    let policy = Policy::of(wanted);
    match wanted.quota {
        Some(quota) if policy.arrives_unseen() => format!(
            "may ask for {} without anybody seeing it first",
            crate::asking::limit(quota)
        ),
        Some(quota) => format!(
            "waits for you, and is counted against {}",
            crate::asking::limit(quota)
        ),
        None => format!("now {}", policy.means()),
    }
}

#[cfg(test)]
mod tests;
