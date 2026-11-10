//! Each wake a phone was sent, by the id it carried, and the phone's own word that it
//! arrived.

use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

use super::{Kept, Record};
use crate::alert::Notification;
use crate::app::Ctx;
use crate::error::codes::notify::NO_SUCH_WAKE;
use crate::error::{Diagnose, Problem, Remedy, State};
use crate::model::{Delivery, DeliveryOutcome};

/// How many random bytes name one wake.
const ID_BYTES: usize = 12;

/// The most wakes kept waiting for their phone's word, and the most failures kept.
const MOST_KEPT: usize = 20;

/// A fresh id for one wake, where the machine supplies the bytes for one.
pub(super) fn fresh(ctx: &Ctx) -> Option<String> {
    ctx.seams
        .random
        .bytes(ID_BYTES)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
}

/// What a wake to `record` carries, as the account of its deliveries names it: that
/// something a member asked for is ready, on a member's own phone, and a problem with the
/// stack on the operator's.
const fn kind_of(record: &Record) -> Notification {
    if record.owner.is_some() {
        Notification::Ready
    } else {
        Notification::Problem
    }
}

/// Write down what became of the wake `id` sent to `name`: waiting for the phone's word
/// where its distributor took it, and failed where it did not.
pub(super) fn sent(path: &Path, name: &str, id: &str, outcome: &DeliveryOutcome, at: u64) {
    let _ = crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        if let Some(one) = kept.endpoints.iter_mut().find(|one| one.name == name) {
            let delivery = Delivery {
                at,
                kind: kind_of(one),
                outcome: outcome.clone(),
            };
            if matches!(outcome, DeliveryOutcome::Failed(_)) {
                one.failures.insert(0, delivery.clone());
                one.failures.truncate(MOST_KEPT);
            } else {
                one.wakes.push(id.to_owned());
                let over = one.wakes.len().saturating_sub(MOST_KEPT);
                one.wakes.drain(..over);
            }
            one.last = Some(delivery);
        }
        Ok(())
    });
}

/// Mark the wake `id` sent to the endpoint `name` confirmed, where `owner` registered it:
/// the operator's own where nobody is named, and the member's where one is.
///
/// # Errors
///
/// A [`Problem`] where no such wake was sent to an endpoint `owner` registered, or the
/// record cannot be read or written.
pub(crate) fn confirmed(
    ctx: &Ctx,
    name: &str,
    id: &str,
    owner: Option<&str>,
) -> Result<super::Listing, Box<Problem>> {
    let path = super::run::kept_at(ctx)?;
    let now = crate::notify::now(ctx);
    let mut found = false;
    let confirm = |kept: &mut Kept| {
        if let Some(one) = kept.endpoints.iter_mut().find(|one| {
            one.name == name && one.owner.as_deref() == owner && one.wakes.iter().any(|w| w == id)
        }) {
            one.wakes.retain(|w| w != id);
            one.last = Some(Delivery {
                at: now,
                kind: kind_of(one),
                outcome: DeliveryOutcome::Confirmed,
            });
            one.failures.clear();
            one.proven = Some(now);
            return true;
        }
        false
    };
    if ctx.dry_run {
        let mut kept = super::run::read_kept(&path)?;
        found = confirm(&mut kept);
    } else {
        crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
            found = confirm(kept);
            Ok(())
        })
        .map_err(|failure| Box::new(failure.problem()))?;
    }
    if !found {
        return Err(Box::new(
            Problem::new(
                NO_SUCH_WAKE,
                format!("No wake by that id was sent to {name}"),
                "Nothing was confirmed.",
                Remedy::new("Confirm the id the wake carried, from the phone it woke"),
            )
            .in_state(State::Guided),
        ));
    }
    super::run::listed_for(ctx, owner)
}

#[cfg(test)]
mod tests;
