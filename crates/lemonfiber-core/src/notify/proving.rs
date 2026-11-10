//! Proving a back-end: a test is sent, and it is proven only once the test is seen arriving.

use super::backends::{self, Backend, Kept};
use super::configuring::{no_such, nowhere, read_failure};
use super::sending::{self, Message};
use crate::alert::Notification;
use crate::app::Ctx;
use crate::error::Problem;
use crate::model::{BackendProof, DeliveryOutcome, NotifyReport};

/// What a test says.
const TEST: &str = "A test from your stack: alerts reach this.";

/// Why a back-end that refused what it is reached with delivers nothing.
pub(crate) const REFUSED_KEY: &str =
    "it refused what it is reached with, so it is proven again before it delivers";

/// Send the back-end `id` a test, and prove it where the test is seen arriving.
///
/// # Errors
///
/// A [`Problem`] where no back-end holds `id`, or the back-ends cannot be read or written.
pub(crate) async fn test(ctx: &Ctx, id: &str) -> Result<NotifyReport, Box<Problem>> {
    let path = backends::at(ctx).ok_or_else(nowhere)?;
    let kept = backends::kept(&path).map_err(|why| read_failure(&why))?;
    let backend = kept
        .backends
        .iter()
        .find(|one| one.id == id)
        .cloned()
        .ok_or_else(|| Box::new(no_such(id)))?;
    if ctx.dry_run {
        return super::reading::report(ctx);
    }
    let proof = proved(ctx, &backend).await;
    backends::changed(&path, &mut |kept: &mut Kept| {
        if let Some(one) = kept.backends.iter_mut().find(|one| one.id == id) {
            one.proof = proof.clone();
        }
        Ok(())
    })
    .map_err(|why| read_failure(&why))?;
    super::reading::report(ctx)
}

/// How far `backend` stands proven once a test is sent to it.
async fn proved(ctx: &Ctx, backend: &Backend) -> BackendProof {
    let held = backends::held(ctx, backend);
    let now = super::now(ctx);
    let message = Message {
        kind: Notification::Completion,
        summary: TEST.to_owned(),
        urgent: false,
        at: now,
    };
    let sent = sending::send(ctx, backend, &held, &message).await;
    if sent.refused_key {
        return BackendProof::Held(REFUSED_KEY.to_owned());
    }
    match (sent.outcome, sent.id) {
        (DeliveryOutcome::Sent, Some(id))
            if sending::arrived(ctx, backend, &held, &id, now.saturating_sub(1)).await =>
        {
            BackendProof::Proven(now)
        }
        _ => BackendProof::Unproven,
    }
}
