//! An operator ruling on an ask for music or books the core keeps, and asking the curator
//! for one once nothing stands in its way.

use std::path::PathBuf;

use super::store::{self, Ask, Standing};
use super::{Collector, Medium, Reference, Unit};
use crate::app::Ctx;
use crate::config::paths::ASKS;
use crate::config::store::Failure;
use crate::error::codes::quota::UNREACHABLE;
use crate::error::{Diagnose, Problem, Remedy};
use crate::household::State;
use crate::model::{MemberRequest, TitleKind, TitleUnit};
use crate::ports::service::{CandidateState, Left};

/// Where the asks are kept.
///
/// # Errors
///
/// Where there is nowhere configured to keep them.
pub(crate) fn kept_at(ctx: &Ctx) -> Result<PathBuf, Box<Problem>> {
    crate::app::targets::beside_env(ctx, ASKS).ok_or_else(|| Box::new(Failure::Nowhere.problem()))
}

/// Every ask kept, and none where nothing is kept or nowhere is configured to keep it.
pub(crate) fn every(ctx: &Ctx) -> Vec<Ask> {
    crate::app::targets::beside_env(ctx, ASKS)
        .and_then(|path| store::kept(&path).ok())
        .map(|kept| kept.asks)
        .unwrap_or_default()
}

/// The curator answering for `medium` in the stack as it stands.
pub(crate) async fn collector(ctx: &Ctx, medium: Medium) -> Option<Collector> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let fillers = crate::app::targets::fillers_here(ctx, &manifest);
    crate::app::targets::collector(ctx, &fillers, medium).await
}

/// Let the waiting ask `medium`-`number` through, asking its curator for it, or turn it
/// down with `reason`; and say what came of it, in the line an operator reads it back in.
///
/// Under a rehearsal, what would come of it, with nothing asked or written.
///
/// # Errors
///
/// A [`Problem`] where no ask by that reference is waiting, where the disk has no room
/// for one let through, where the curator cannot be asked or would not take it, and
/// where the asks or the reasons cannot be written.
pub(crate) async fn decided(
    ctx: &Ctx,
    medium: Medium,
    number: u64,
    reason: Option<&str>,
) -> Result<String, Box<Problem>> {
    let named = Reference::Kept(medium, number);
    let path = kept_at(ctx)?;
    let kept = store::kept(&path).map_err(|why| Box::new(why.problem()))?;
    let waiting = kept
        .asks
        .into_iter()
        .find(|ask| ask.named() == named && ask.standing == Standing::Waiting)
        .ok_or_else(|| Box::new(crate::asking::nothing_to_decide(named)))?;
    if reason.is_none() {
        crate::space::run::admits(ctx).await?;
    }
    let said = said_of(&waiting, reason);
    if ctx.dry_run {
        return Ok(format!(
            "{said} — rehearsed, and nothing was sent or decided"
        ));
    }
    let at = crate::instant::written(ctx.seams.clock.now()).unwrap_or_default();
    let standing = match reason {
        None => {
            let curator = collector(ctx, medium)
                .await
                .ok_or_else(|| Box::new(unreached(NOTHING_DECIDED)))?;
            sent(&curator, waiting.unit, &waiting.reference).await?;
            Standing::Sent
        }
        Some(reason) => {
            let mut reasons = crate::app::refusals::load(ctx);
            reasons.keep(named, reason, Some(at.clone()));
            crate::app::refusals::keep(ctx, &reasons)?;
            Standing::Declined
        }
    };
    store::settled(&path, named, standing, &at).map_err(|why| Box::new(why.problem()))?;
    Ok(said)
}

/// Close every ask the core keeps that has waited `after` days or more with nobody ruling
/// on it, keeping `reason` as why, and say how many; under a rehearsal, how many it would.
///
/// An ask whose date cannot be read is never closed: it has not been shown to have waited.
///
/// # Errors
///
/// What stopped the asks or the reasons being read or written, as the line a run reads.
pub(crate) fn expired(ctx: &Ctx, after: u32, reason: &str) -> Result<usize, String> {
    let Some(path) = crate::app::targets::beside_env(ctx, ASKS) else {
        return Ok(0);
    };
    let now = ctx.seams.clock.now();
    let overdue: Vec<Reference> = store::kept(&path)
        .map_err(|why| why.to_string())?
        .asks
        .iter()
        .filter(|ask| ask.standing == Standing::Waiting)
        .filter(|ask| {
            crate::asking::waiting_for(Some(&ask.asked_at), now)
                .is_some_and(|days| days >= u64::from(after))
        })
        .map(Ask::named)
        .collect();
    if ctx.dry_run || overdue.is_empty() {
        return Ok(overdue.len());
    }
    let at = crate::instant::written(now).unwrap_or_default();
    let mut reasons = crate::app::refusals::load(ctx);
    for named in &overdue {
        store::settled(&path, *named, Standing::Expired, &at).map_err(|why| why.to_string())?;
        reasons.closed(*named, reason, Some(at.clone()));
    }
    crate::app::refusals::keep(ctx, &reasons).map_err(|why| why.summary)?;
    Ok(overdue.len())
}

/// What the member `member` may still ask for of `medium`: the limit and period `films`,
/// what the household holds them to for a film, give, each ask they made of `medium`
/// inside that period counted once, and none that was turned down or closed.
pub(crate) fn left(
    kept: &[Ask],
    films: Left,
    member: &str,
    medium: Medium,
    now: std::time::SystemTime,
) -> Left {
    let used = films.days.map_or(0, |days| {
        kept.iter()
            .filter(|ask| ask.member == member && ask.unit.medium() == medium)
            .filter(|ask| {
                matches!(
                    ask.standing,
                    Standing::Waiting | Standing::Sent | Standing::PartlyHere | Standing::Here
                )
            })
            .filter(|ask| {
                crate::asking::waiting_for(Some(&ask.asked_at), now)
                    .is_some_and(|age| age < u64::from(days))
            })
            .count()
    });
    Left {
        limit: films.limit,
        used: u32::try_from(used).unwrap_or(u32::MAX),
        days: films.days,
    }
}

/// Read each sent ask back from its curator, and mark those the house now holds, all of
/// them or in part, with when the core first saw them held. Nothing under a rehearsal, and
/// an ask whose curator does not answer stays where it stood.
pub(crate) async fn arrivals(ctx: &Ctx) {
    let Some(path) = crate::app::targets::beside_env(ctx, ASKS) else {
        return;
    };
    if ctx.dry_run {
        return;
    }
    let waiting: Vec<Ask> = every(ctx)
        .into_iter()
        .filter(|ask| matches!(ask.standing, Standing::Sent | Standing::PartlyHere))
        .collect();
    for medium in Medium::ALL {
        let of_medium: Vec<&Ask> = waiting
            .iter()
            .filter(|ask| ask.unit.medium() == medium)
            .collect();
        if of_medium.is_empty() {
            continue;
        }
        let Some(curator) = collector(ctx, medium).await else {
            continue;
        };
        for ask in of_medium {
            let held = match curator.candidate(ask.unit, &ask.reference).await {
                Ok(Some(found)) if found.state == CandidateState::Here => Standing::Here,
                Ok(Some(found)) if found.state == CandidateState::PartlyHere => {
                    Standing::PartlyHere
                }
                _ => continue,
            };
            let at = crate::instant::written(ctx.seams.clock.now()).unwrap_or_default();
            if let Ok(Some(member)) = store::arrived(&path, ask.named(), held, &at) {
                crate::notify::fanning::ready(ctx, &member);
                crate::push::telling::ready(ctx, &member);
            }
        }
    }
}

/// Ask `curator` for the candidate `reference` names, as a `unit`.
///
/// # Errors
///
/// The refusal for a curator that would not answer or would not take it.
pub(crate) async fn sent(
    curator: &Collector,
    unit: Unit,
    reference: &str,
) -> Result<(), Box<Problem>> {
    curator
        .request(unit, reference)
        .await
        .map(|_| ())
        .map_err(|_| Box::new(unreached(NOTHING_DECIDED)))
}

/// `ask` as a line of the household list, with the reason it was turned down where
/// `reasons` holds one.
pub(crate) fn listed(
    ask: &Ask,
    reasons: &crate::asking::Reasons,
    now: std::time::SystemTime,
) -> MemberRequest {
    let state = match ask.standing {
        Standing::Waiting => State::WaitingForApproval,
        Standing::Sent => State::Getting,
        Standing::PartlyHere => State::PartlyHere,
        Standing::Here => State::Here,
        Standing::Declined | Standing::Expired => State::Declined,
    };
    MemberRequest {
        id: ask.named().to_string(),
        asked: Some(ask.asked_at.clone()),
        title: Some(ask.name.clone()),
        year: ask.year,
        arrived: ask.arrived_at.clone(),
        shelf_id: None,
        media: Some(ask.unit.noun().to_owned()),
        kind: Some(TitleKind::from(ask.unit.medium())),
        unit: Some(TitleUnit::from(ask.unit)),
        state: Some(state),
        waiting_days: (state == State::WaitingForApproval)
            .then(|| crate::asking::waiting_for(Some(&ask.asked_at), now))
            .flatten(),
        estimate: None,
        refused: reasons.of(ask.named()).cloned(),
    }
}

/// What is said where the curator could not be asked or would not take the ask.
const NOTHING_DECIDED: &str = "nothing was decided";

/// Said where the curator of music or books could not be asked or would not answer.
fn unreached(doing: &str) -> Problem {
    Problem::new(
        UNREACHABLE,
        format!("the curator of music or books would not answer, so {doing}"),
        "An ask the core keeps goes to the curator once it is let through, and the curator \
         did not take it, so nothing was changed: the ask still waits where it waited",
        Remedy::new("Check the curator is running, then run this again"),
    )
}

/// What the decision comes to, as the line an operator reads it back in.
fn said_of(waiting: &Ask, reason: Option<&str>) -> String {
    let what = &waiting.name;
    match reason {
        None => format!("{what} was approved and is being fetched"),
        Some(reason) => format!(
            "{what} was turned down: {reason} — kept here, and read on the household list \
             by whoever asked"
        ),
    }
}

#[cfg(test)]
mod tests;
