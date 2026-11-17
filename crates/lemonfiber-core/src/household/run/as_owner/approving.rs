//! An operator ruling on an ask kept until it is filed as the request service's owner,
//! closing those left waiting too long, and listing them on the household list.

use std::path::PathBuf;

use lemonfiber_contract::capabilities::request::intake::Fills;

use super::{Kept, Waiting};
use crate::app::Ctx;
use crate::asking::NOTHING_DECIDED;
use crate::collected::store::Standing;
use crate::collected::Reference;
use crate::config::paths::ASKED_AS_OWNER;
use crate::config::store::Failure;
use crate::error::{Diagnose, Problem};
use crate::household::State;
use crate::instant::Instant;
use crate::model::{MemberRequest, TitleKind};
use crate::ports::media::Kind;
use crate::ports::service::Wish;
use crate::quality::Selection;

/// Where the asks are kept.
///
/// # Errors
///
/// Where there is nowhere configured to keep them.
fn kept_at(ctx: &Ctx) -> Result<PathBuf, Box<Problem>> {
    crate::app::targets::beside_env(ctx, ASKED_AS_OWNER)
        .ok_or_else(|| Box::new(Failure::Nowhere.problem()))
}

/// Let the waiting ask `kind`-`number` through, filing it with `requests` as the owner and
/// keeping the request beside its member, or turn it down with `reason`; and say what came
/// of it, in the line an operator reads it back in.
///
/// Under a rehearsal, what would come of it, with nothing filed or written.
///
/// # Errors
///
/// A [`Problem`] where no ask by that reference is waiting, where the disk has no room
/// for one let through, where the request service cannot be read or would not file it,
/// and where the asks or the reasons cannot be written.
pub(crate) async fn decided(
    ctx: &Ctx,
    requests: &dyn Fills,
    (kind, number): (Kind, u64),
    reason: Option<&str>,
) -> Result<String, Box<Problem>> {
    let named = Reference::AsOwner(kind, number);
    let path = kept_at(ctx)?;
    let waiting = super::read(&path)
        .map_err(|why| Box::new(why.problem()))?
        .waiting
        .into_iter()
        .find(|ask| ask.named() == named && ask.waits())
        .ok_or_else(|| Box::new(crate::asking::nothing_to_decide(named)))?;
    if reason.is_none() {
        crate::space::run::admits(ctx).await?;
    }
    let said = crate::collected::run::said_of(&what(&waiting), reason);
    if ctx.dry_run {
        return Ok(format!(
            "{said} — rehearsed, and nothing was sent or decided"
        ));
    }
    let unreached = || Box::new(crate::asking::unreachable(NOTHING_DECIDED));
    let at = ctx.stamp();
    match reason {
        None => {
            let standing = requests.requests().await.map_err(|_| unreached())?;
            let asked = requests
                .ask(
                    None,
                    &Wish {
                        id: waiting.title.clone(),
                        kind,
                        seasons: waiting.seasons.clone(),
                    },
                )
                .await
                .map_err(|_| unreached())?;
            super::approved(&path, named, asked.request, (&standing, at), at)
                .map_err(|why| Box::new(why.problem()))?;
        }
        Some(reason) => {
            let mut reasons = crate::app::refusals::load(ctx);
            reasons.keep(named, reason, Some(at));
            crate::app::refusals::keep(ctx, &reasons)?;
            super::settled(&path, named, Standing::Declined, at)
                .map_err(|why| Box::new(why.problem()))?;
        }
    }
    Ok(said)
}

/// Close every ask kept here that has waited `after` days or more with nobody ruling on
/// it, keeping `reason` as why, and say how many; under a rehearsal, how many it would.
///
/// # Errors
///
/// What stopped the asks or the reasons being read or written, as the line a run reads.
pub(crate) fn expired(ctx: &Ctx, after: u32, reason: &str) -> Result<usize, String> {
    let Some(path) = crate::app::targets::beside_env(ctx, ASKED_AS_OWNER) else {
        return Ok(0);
    };
    let now = ctx.stamp();
    let overdue: Vec<Reference> = super::read(&path)
        .map_err(|why| why.to_string())?
        .waiting
        .iter()
        .filter(|ask| ask.waits() && crate::collected::run::overstayed(ask.asked_at, after, now))
        .map(Waiting::named)
        .collect();
    crate::collected::run::closing(ctx, (&overdue, now), reason, &mut |named| {
        super::settled(&path, named, Standing::Expired, now).map(|_| ())
    })
}

/// Each ask `kept` holds that is still the core's to show, as lines of the household
/// list, one for each member on it, the newest first: those waiting and those turned
/// down or closed, but none filed, which the request service lists itself.
pub(crate) fn listed<'a>(
    kept: &'a Kept,
    reasons: &'a crate::asking::Reasons,
    quality: &'a Selection,
    now: Instant,
) -> impl Iterator<Item = (&'a str, MemberRequest)> + 'a {
    kept.waiting
        .iter()
        .rev()
        .filter(|ask| ask.standing != Standing::Sent)
        .flat_map(move |ask| {
            ask.members()
                .into_iter()
                .map(|(member, _, asked_at)| (member, line(ask, asked_at, reasons, quality, now)))
                .collect::<Vec<_>>()
        })
}

/// `ask` as the line of a member on it who first asked at `asked_at`.
fn line(
    ask: &Waiting,
    asked_at: Instant,
    reasons: &crate::asking::Reasons,
    quality: &Selection,
    now: Instant,
) -> MemberRequest {
    let state = if ask.waits() {
        State::WaitingForApproval
    } else {
        State::Declined
    };
    MemberRequest {
        id: ask.named().to_string(),
        asked: Some(asked_at),
        title: ask.called.clone(),
        year: ask.year,
        arrived: None,
        shelf_id: None,
        media: Some(ask.kind.noun().to_owned()),
        kind: Some(TitleKind::from(ask.kind)),
        unit: None,
        state: Some(state),
        waiting_days: crate::household::run::allowance::waiting(Some(state), Some(asked_at), now),
        estimate: crate::household::run::allowance::estimated(Some(ask.kind), quality),
        refused: reasons.of(ask.named()).cloned(),
        also_asked: Vec::new(),
    }
}

/// What `ask` is for, in the words a decision is read back in: the title by name where
/// the request service named it, and otherwise what every member on it asked for.
fn what(ask: &Waiting) -> String {
    ask.called.clone().unwrap_or_else(|| {
        let names: Vec<&str> = ask.members().into_iter().map(|(_, name, _)| name).collect();
        format!("what {} asked for", names.join(" and "))
    })
}

#[cfg(test)]
mod tests;
