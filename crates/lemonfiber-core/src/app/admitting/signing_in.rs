//! A password exchanged for a session: the machine's own, a member's, or one chosen at a
//! claim.

use std::net::IpAddr;
use std::time::{Duration, SystemTime};

use super::attempts::{Door, Ticket};
use super::household::{claimable, named, opened, vouched};
use super::refused::{no_session, not_open, not_the_password, short_choice};
use super::sessions::{Admitted, Opened};
use super::Caller;
use crate::app::Ctx;
use crate::error::Problem;
use crate::ports::random::Random;
use crate::ports::service::Household;

/// How many unpredictable bytes name one sign-in at the media server.
const DEVICE_BYTES: usize = 16;

/// What a caller offers at the door.
#[derive(Clone, PartialEq, Eq)]
pub struct Offered {
    /// Who they say they are; absent from the operator, who signs in with the machine's
    /// own password.
    pub name: Option<String>,
    /// The claim token an invitation's join link carries, where this is a claim.
    pub claim: Option<String>,
    /// The address the request came from, where it came over a socket.
    pub peer: Option<IpAddr>,
}

impl std::fmt::Debug for Offered {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Offered")
            .field("name", &self.name)
            .field("claiming", &self.claim.is_some())
            .field("peer", &self.peer)
            .finish()
    }
}

/// What the asker is asked for the password they offer.
pub const PASSWORD: &str = "password";

/// The session `offered` opens, with the password the asker gives.
pub(super) async fn signed_in(ctx: &Ctx, offered: &Offered) -> Result<Admitted, Box<Problem>> {
    let password = ctx.asking.secret(PASSWORD);
    if offered.claim.is_some() && password.chars().count() < crate::admission::LEAST {
        return Err(short_choice());
    }
    let admitting = &ctx.admitting;
    let now = ctx.seams.clock.now();
    let ticket = admitting
        .attempts
        .taken(offered.peer, offered.name.as_deref(), now)
        .await
        .map_err(Caller::held)?;
    let random = ctx.seams.random.as_ref();
    let (who, door) = match offered.claim.as_deref() {
        Some(token) => claimed(ctx, offered, &password, token, &ticket, random).await?,
        None => whoever(ctx, offered.name.as_deref(), &password, &ticket, random)
            .await
            .ok_or_else(not_the_password)?,
    };
    admitting.attempts.right(&ticket, door, now).await;
    let name = match &who {
        Opened::Member(signed) => Some(
            named(ctx, &signed.id)
                .await
                .ok_or_else(Caller::unconfirmed)?,
        ),
        Opened::Operator(_) => None,
    };
    admitting
        .sessions
        .opened(random, now, who)
        .await
        .map(|opened| opened.called(name))
        .ok_or_else(no_session)
}

/// Who a name and a password prove somebody to be: the machine's own password first, then
/// the household, each only where the ticket leaves its door open.
async fn whoever(
    ctx: &Ctx,
    name: Option<&str>,
    password: &str,
    ticket: &Ticket,
    random: &dyn Random,
) -> Option<(Opened, Door)> {
    if ticket.operator {
        if let Some(held) = super::credential_now(ctx).await {
            let typed = password.to_owned();
            let proved = tokio::task::spawn_blocking(move || held.verifies(&typed).then_some(held))
                .await
                .ok()
                .flatten();
            if let Some(held) = proved {
                return Some((Opened::Operator(held), Door::Operator));
            }
        }
    }
    let door = ticket.member.clone()?;
    let name = name?;
    if password.is_empty() {
        return None;
    }
    let household = opened(ctx).await?;
    member_signed_in(ctx, household.as_ref(), name, password, random)
        .await
        .map(|opened| (opened, door))
}

/// The session a claim on an invitation opens, having set the password chosen.
async fn claimed(
    ctx: &Ctx,
    offered: &Offered,
    password: &str,
    token: &str,
    ticket: &Ticket,
    random: &dyn Random,
) -> Result<(Opened, Door), Box<Problem>> {
    let door = ticket.member.clone().ok_or_else(not_open)?;
    let asked = offered.name.as_deref().ok_or_else(not_open)?.to_lowercase();
    let household = opened(ctx).await.ok_or_else(Caller::unconfirmed)?;
    let member = household
        .household()
        .await
        .map_err(|_| Caller::unconfirmed())?
        .into_iter()
        .find(|member| member.name.to_lowercase() == asked)
        .filter(|member| !(member.claimed || member.access.disabled || member.access.administrator))
        .ok_or_else(not_open)?;
    if !claimable(ctx, &member.id, token).await {
        return Err(not_open());
    }
    let device = device(random).ok_or_else(Caller::unconfirmed)?;
    let set = household
        .claim(&member.name, password, &device)
        .await
        .map_err(|_| Caller::unconfirmed())?;
    if !set {
        return Err(not_open());
    }
    ctx.admitting.household.claim_spent(ctx, &member.id);
    member_signed_in(ctx, household.as_ref(), &member.name, password, random)
        .await
        .map(|opened| (opened, door))
        .ok_or_else(Caller::unconfirmed)
}

/// A fresh name for one sign-in at the media server.
pub(super) fn device(random: &dyn Random) -> Option<String> {
    random
        .bytes(DEVICE_BYTES)
        .map(|bytes| crate::secret::render(&bytes))
}

/// A member signed in at the household, where it vouches for them.
async fn member_signed_in(
    ctx: &Ctx,
    household: &dyn Household,
    name: &str,
    password: &str,
    random: &dyn Random,
) -> Option<Opened> {
    let device = device(random)?;
    let signed = household
        .whoever(name, password, &device)
        .await
        .ok()
        .flatten()?;
    vouched(ctx, &signed.id)
        .await
        .then_some(Opened::Member(signed))
}

/// Whether `password` is the machine's own, counted as an answer from `peer`.
pub(crate) async fn proves_the_operator(
    ctx: &Ctx,
    password: &str,
    peer: Option<IpAddr>,
    now: SystemTime,
) -> Result<bool, Duration> {
    let attempts = &ctx.admitting.attempts;
    let ticket = attempts.taken(peer, None, now).await?;
    let Some(held) = super::credential_now(ctx).await.filter(|_| ticket.operator) else {
        return Ok(false);
    };
    let typed = password.to_owned();
    let proved = tokio::task::spawn_blocking(move || held.verifies(&typed))
        .await
        .unwrap_or(false);
    if proved {
        attempts.right(&ticket, Door::Operator, now).await;
    }
    Ok(proved)
}

/// Whether `password` is member `id`'s, counted as an answer from `peer`.
pub(crate) async fn proves_the_member(
    ctx: &Ctx,
    id: &str,
    password: &str,
    peer: Option<IpAddr>,
    now: SystemTime,
) -> Result<bool, Duration> {
    let Some(name) = named(ctx, id).await else {
        return Ok(false);
    };
    let attempts = &ctx.admitting.attempts;
    let ticket = attempts.taken(peer, Some(&name), now).await?;
    let random = ctx.seams.random.as_ref();
    let proved = match whoever(ctx, Some(&name), password, &ticket, random).await {
        Some((Opened::Member(signed), door)) if signed.id == id => Some(door),
        _ => None,
    };
    let Some(door) = proved else {
        return Ok(false);
    };
    attempts.right(&ticket, door, now).await;
    Ok(true)
}
