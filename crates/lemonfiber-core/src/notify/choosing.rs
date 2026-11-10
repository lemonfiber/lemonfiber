//! The ways a member may be told that something they asked for is ready, and their choice
//! among them.

use super::backends::{self, Kept};
use super::configuring::{nowhere, read_failure};
use crate::app::Ctx;
use crate::error::codes::notify::NOT_OFFERED;
use crate::error::{Problem, Remedy, State};
use crate::model::{NotifyMineReport, OfferedBackend};

/// What the operator offered the household, with what `member` chose and whether their
/// own phone is registered.
///
/// # Errors
///
/// A [`Problem`] where there is nowhere to keep back-ends, or their record cannot be read.
pub(crate) fn mine(ctx: &Ctx, member: &str) -> Result<NotifyMineReport, Box<Problem>> {
    let path = backends::at(ctx).ok_or_else(nowhere)?;
    let kept = backends::kept(&path).map_err(|why| read_failure(&why))?;
    Ok(NotifyMineReport {
        offered: kept
            .backends
            .iter()
            .filter_map(|one| {
                one.members.as_ref().map(|name| OfferedBackend {
                    id: one.id.clone(),
                    name: name.clone(),
                    receiving: one.receivers.iter().any(|receiver| receiver == member),
                })
            })
            .collect(),
        phone: crate::push::telling::owns_one(ctx, member),
        rehearsed: ctx.dry_run,
    })
}

/// Tell `member` on each of `chosen` from now on, and on no other, each one the operator
/// offered the household.
///
/// # Errors
///
/// A [`Problem`] where one chosen was not offered, there is nowhere to keep back-ends, or
/// their record cannot be read or written.
pub(crate) fn choose(
    ctx: &Ctx,
    member: &str,
    chosen: &[String],
) -> Result<NotifyMineReport, Box<Problem>> {
    let path = backends::at(ctx).ok_or_else(nowhere)?;
    let kept = backends::kept(&path).map_err(|why| read_failure(&why))?;
    if let Some(unoffered) = chosen.iter().find(|id| {
        !kept
            .backends
            .iter()
            .any(|one| one.id == **id && one.members.is_some())
    }) {
        return Err(Box::new(
            Problem::new(
                NOT_OFFERED,
                format!("{unoffered} is not one of the ways offered to be told"),
                "Nothing was changed.",
                Remedy::new("Choose from the ones offered"),
            )
            .in_state(State::Guided),
        ));
    }
    if !ctx.dry_run {
        backends::changed(&path, &mut |kept: &mut Kept| {
            for one in kept.backends.iter_mut().filter(|one| one.members.is_some()) {
                one.receivers.retain(|receiver| receiver != member);
                if chosen.contains(&one.id) {
                    one.receivers.push(member.to_owned());
                }
            }
            Ok(())
        })
        .map_err(|why| read_failure(&why))?;
    }
    mine(ctx, member)
}

#[cfg(test)]
mod tests;
