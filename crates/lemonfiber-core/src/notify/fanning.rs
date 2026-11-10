//! Each new alert delivered to every back-end that is not held, each on its own, so one
//! that fails or hangs costs the others nothing.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::backends::{self, Backend, Held, Kept, Owed};
use super::sending::{self, Message, Sent};
use crate::alert::{Alert, Notification};
use crate::app::Ctx;
use crate::error::Severity;
use crate::model::{BackendKind, BackendProof, ChosenBy, Delivery, DeliveryOutcome, DeliveryRetry};
use crate::push::run::said_as;

/// The most messages owed to one back-end; the oldest are delivered first and the newest
/// past this are not owed.
const MOST_OWED: usize = 50;

/// The most failed deliveries a back-end keeps.
const MOST_FAILURES: usize = 20;

/// All a member is told: that something they asked for is ready.
pub const READY: &str = "Something you asked for is ready";

/// Seconds in a minute, which the delays are given in.
const MINUTE: u64 = 60;

/// Whether any back-end is configured, so the stack is read for alerts with nobody
/// watching it.
#[must_use]
pub fn any_configured(ctx: &Ctx) -> bool {
    configured_at(ctx).is_some()
}

/// Where the back-ends are kept, where any is.
fn configured_at(ctx: &Ctx) -> Option<PathBuf> {
    backends::at(ctx)
        .filter(|path| backends::kept(path).is_ok_and(|kept| !kept.backends.is_empty()))
}

/// The deliveries under way, one to a back-end, each apart from whatever reads the stack.
#[derive(Debug, Clone, Default)]
pub struct Fanning {
    /// The id of each back-end a delivery is under way to.
    delivering: Arc<Mutex<BTreeSet<String>>>,
}

impl Fanning {
    /// Owe every back-end that receives the stack's alerts one message for each alert in
    /// `alerts` that was not carried at the last reading, and start delivering to each one
    /// owed something now. Waits on no back-end.
    pub fn owing(&self, ctx: &Arc<Ctx>, alerts: &[Alert]) {
        let Some(path) = configured_at(ctx) else {
            return;
        };
        let now = super::now(ctx);
        let mut due = Vec::new();
        let written = backends::changed(&path, &mut |kept: &mut Kept| {
            owe(kept, alerts, now);
            due = kept
                .backends
                .iter()
                .filter(|one| is_due(one, now))
                .map(|one| one.id.clone())
                .collect();
            Ok(())
        });
        for id in due.into_iter().filter(|_| written.is_ok()) {
            self.delivering_to(ctx, &path, id);
        }
    }

    /// Deliver what `id` is owed, unless a delivery to it is already under way.
    fn delivering_to(&self, ctx: &Arc<Ctx>, path: &Path, id: String) {
        let started = self
            .delivering
            .lock()
            .is_ok_and(|mut delivering| delivering.insert(id.clone()));
        if started {
            let delivering = Arc::clone(&self.delivering);
            let ctx = Arc::clone(ctx);
            let path = path.to_path_buf();
            tokio::spawn(async move {
                delivered(&ctx, &path, &id).await;
                let _ = delivering
                    .lock()
                    .map(|mut delivering| delivering.remove(&id));
            });
        }
    }
}

/// Owe each back-end that receives the stack's alerts every alert `kept` had not told,
/// and remember every alert carried; at the first reading, nothing is owed.
fn owe(kept: &mut Kept, alerts: &[Alert], now: u64) {
    let fresh: Vec<&Alert> = kept.told.as_ref().map_or_else(Vec::new, |told| {
        alerts
            .iter()
            .filter(|alert| said_as(alert).is_some_and(|said| !told.contains(&said)))
            .collect()
    });
    kept.told = Some(alerts.iter().filter_map(said_as).collect());
    for one in kept
        .backends
        .iter_mut()
        .filter(|one| receives_the_stack(one))
    {
        for alert in &fresh {
            if one.owed.len() < MOST_OWED {
                one.owed.push(Owed {
                    kind: alert.notification,
                    summary: alert.said(),
                    urgent: alert.severity >= Severity::Error,
                    at: now,
                    attempts: 0,
                    next: None,
                });
            }
        }
    }
}

/// Whether `backend` is told the stack's own alerts: the operator's own and not offered to
/// the household, whose members are told only that something they asked for is ready, and
/// never a phone's push endpoint, which is woken apart from these.
fn receives_the_stack(backend: &Backend) -> bool {
    backend.chosen_by == ChosenBy::Operator
        && backend.members.is_none()
        && backend.kind != BackendKind::Unifiedpush
}

/// Owe every back-end that receives the stack's alerts one message now, apart from any
/// alert the dashboard carries: what a service told the core happened.
pub(crate) fn owe_now(ctx: &Ctx, kind: Notification, summary: &str, urgent: bool) {
    let Some(path) = configured_at(ctx) else {
        return;
    };
    let now = super::now(ctx);
    let _ = backends::changed(&path, &mut |kept: &mut Kept| {
        for one in kept
            .backends
            .iter_mut()
            .filter(|one| receives_the_stack(one) && one.owed.len() < MOST_OWED)
        {
            one.owed.push(Owed {
                kind,
                summary: summary.to_owned(),
                urgent,
                at: now,
                attempts: 0,
                next: None,
            });
        }
        Ok(())
    });
}

/// Owe every back-end `member` receives on that something they asked for is ready, and
/// nothing more: no title, no service and nothing about the stack.
pub(crate) fn ready(ctx: &Ctx, member: &str) {
    let Some(path) = configured_at(ctx) else {
        return;
    };
    let now = super::now(ctx);
    let _ = backends::changed(&path, &mut |kept: &mut Kept| {
        for one in kept.backends.iter_mut().filter(|one| {
            one.members.is_some()
                && one.receivers.iter().any(|receiver| receiver == member)
                && one.owed.len() < MOST_OWED
        }) {
            one.owed.push(Owed {
                kind: Notification::Ready,
                summary: READY.to_owned(),
                urgent: false,
                at: now,
                attempts: 0,
                next: None,
            });
        }
        Ok(())
    });
}

/// Whether `backend` has a message to deliver now.
fn is_due(backend: &Backend, now: u64) -> bool {
    !matches!(backend.proof, BackendProof::Held(_))
        && backend
            .owed
            .first()
            .is_some_and(|owed| owed.next.is_none_or(|next| next <= now))
}

/// Send `id` everything it is owed, oldest first, until one fails or nothing is due.
async fn delivered(ctx: &Ctx, path: &Path, id: &str) {
    while let Some((backend, owed)) = next_due(path, id, super::now(ctx)) {
        let held = backends::held(ctx, &backend);
        let message = Message {
            kind: owed.kind,
            summary: owed.summary.clone(),
            urgent: owed.urgent,
            at: owed.at,
        };
        let sent = sending::send(ctx, &backend, &held, &message).await;
        let outcome = confirmed(ctx, &backend, &held, &sent, owed.at).await;
        let failed = matches!(outcome, DeliveryOutcome::Failed(_));
        let _ = backends::changed(path, &mut |kept: &mut Kept| {
            if let Some(one) = kept.backends.iter_mut().find(|one| one.id == id) {
                settled(one, &owed, &outcome, sent.refused_key, super::now(ctx));
            }
            Ok(())
        });
        if failed {
            return;
        }
    }
}

/// What became of a message `sent` to `backend`: confirmed where its history shows it
/// arrived.
async fn confirmed(
    ctx: &Ctx,
    backend: &Backend,
    held: &Held,
    sent: &Sent,
    since: u64,
) -> DeliveryOutcome {
    match (&sent.outcome, &sent.id) {
        (DeliveryOutcome::Sent, Some(id))
            if sending::arrived(ctx, backend, held, id, since.saturating_sub(1)).await =>
        {
            DeliveryOutcome::Confirmed
        }
        (outcome, _) => outcome.clone(),
    }
}

/// `id`'s record and the message it is to be sent next, where one is due.
fn next_due(path: &Path, id: &str, now: u64) -> Option<(Backend, Owed)> {
    backends::kept(path)
        .ok()?
        .backends
        .into_iter()
        .find(|one| one.id == id && is_due(one, now))
        .and_then(|one| one.owed.first().cloned().map(|owed| (one, owed)))
}

/// Write down what became of sending `owed` to `backend`: delivered, tried again later,
/// standing failed once its attempts are spent, or held where it refused its secret.
fn settled(backend: &mut Backend, owed: &Owed, outcome: &DeliveryOutcome, refused: bool, now: u64) {
    let delivery = Delivery {
        at: now,
        kind: owed.kind,
        outcome: outcome.clone(),
    };
    backend.last = Some(delivery.clone());
    if refused {
        backend.proof = BackendProof::Held(super::proving::REFUSED_KEY.to_owned());
    }
    let Some(head) = backend.owed.first_mut().filter(|head| *head == owed) else {
        return;
    };
    match outcome {
        DeliveryOutcome::Failed(_) => {
            backend.failures.insert(0, delivery);
            backend.failures.truncate(MOST_FAILURES);
            if refused {
                return;
            }
            head.attempts += 1;
            let delay = usize::try_from(head.attempts - 1)
                .ok()
                .and_then(|at| backend.delays.get(at));
            if let Some(minutes) = delay.filter(|_| head.attempts < backend.attempts) {
                let next = now.saturating_add(minutes.saturating_mul(MINUTE));
                head.next = Some(next);
                backend.retry = Some(DeliveryRetry {
                    next,
                    attempt: head.attempts + 1,
                    of: backend.attempts,
                });
            } else {
                backend.owed.remove(0);
                backend.retry = None;
            }
        }
        DeliveryOutcome::Confirmed | DeliveryOutcome::Sent => {
            if *outcome == DeliveryOutcome::Confirmed {
                backend.failures.clear();
            }
            backend.owed.remove(0);
            backend.retry = None;
        }
    }
}

#[cfg(test)]
mod tests;
