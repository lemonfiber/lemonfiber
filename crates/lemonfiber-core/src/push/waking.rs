//! Waking the phones that asked to be woken: each one owed a message for each alert, sent
//! apart from every other phone's, each carrying an id its phone confirms it by.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

use super::run::{said_as, waiting_at, GONE, HELD_FOR_SECONDS, MOST_OWED, WAITED_FOR};
use super::{sealing, Kept, Record};
use crate::alert::Alert;
use crate::app::Ctx;
use crate::model::DeliveryOutcome;
use crate::ports::http::{Method, Request};

/// The deliveries under way, one to an endpoint, each apart from whatever reads the stack
/// and from every other endpoint's, so one that hangs delays nothing but itself.
#[derive(Debug, Clone, Default)]
pub struct Waking {
    /// The name of each endpoint a delivery is under way to.
    delivering: Arc<Mutex<BTreeSet<String>>>,
}

impl Waking {
    /// Owe every endpoint one message for each alert in `alerts` none has been owed, and
    /// start delivering to each endpoint owed any. Waits on no endpoint.
    pub fn owing(&self, ctx: &Arc<Ctx>, alerts: &[Alert]) {
        let Some(path) = waiting_at(ctx) else {
            return;
        };
        let carried: BTreeSet<String> = alerts.iter().filter_map(said_as).collect();
        let mut owing = Vec::new();
        let written = crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
            let new = kept.told.as_ref().map_or(0, |told| {
                carried.iter().filter(|one| !told.contains(one)).count()
            });
            kept.told = Some(carried.iter().cloned().collect());
            for one in kept.endpoints.iter_mut().filter(|one| one.owner.is_none()) {
                one.owed = one.owed.saturating_add(new).min(MOST_OWED);
            }
            owing = kept
                .endpoints
                .iter()
                .filter(|one| one.owed > 0)
                .map(|one| one.name.clone())
                .collect();
            Ok(())
        });
        for name in owing.into_iter().filter(|_| written.is_ok()) {
            self.delivering_to(ctx, &path, name);
        }
    }

    /// Deliver what `name` is owed, unless a delivery to it is already under way.
    ///
    /// One that has just found nothing owed can miss what is owed a moment later, and the
    /// next reading starts another for it.
    fn delivering_to(&self, ctx: &Arc<Ctx>, path: &Path, name: String) {
        let started = self
            .delivering
            .lock()
            .is_ok_and(|mut delivering| delivering.insert(name.clone()));
        if started {
            let delivering = Arc::clone(&self.delivering);
            let ctx = Arc::clone(ctx);
            let path = path.to_path_buf();
            tokio::spawn(async move {
                delivered(&ctx, &path, &name).await;
                let _ = delivering
                    .lock()
                    .map(|mut delivering| delivering.remove(&name));
            });
        }
    }
}

/// Send `name` everything it is owed, one message at a time, each carrying an id its phone
/// confirms it by, until its distributor answers that it is gone: the endpoint stays, the
/// failure shown, until the phone or the operator removes it.
async fn delivered(ctx: &Ctx, path: &Path, name: &str) {
    while let Some(record) = next_owed(path, name) {
        let Some(id) = super::woken::fresh(ctx) else {
            return;
        };
        let (outcome, gone) = woke(ctx, &record, &id).await;
        super::woken::sent(path, name, &id, &outcome, crate::notify::now(ctx));
        if gone {
            return;
        }
    }
}

/// `name`'s record, with the message about to be sent no longer owed: nothing where it owes
/// none, has been removed, or the record cannot be written, so nothing is sent twice.
fn next_owed(path: &Path, name: &str) -> Option<Record> {
    let mut next = None;
    let written = crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        next = kept
            .endpoints
            .iter_mut()
            .find(|one| one.name == name && one.owed > 0)
            .map(|one| {
                one.owed -= 1;
                one.clone()
            });
        Ok(())
    });
    written.ok().and(next)
}

/// The message that wakes `record` carrying `id`, where its keys can be read.
fn sealed_for(record: &Record, id: &str) -> Option<Vec<u8>> {
    let public = URL_SAFE_NO_PAD.decode(&record.p256dh).ok()?;
    let secret = URL_SAFE_NO_PAD.decode(&record.auth).ok()?;
    sealing::sealed(&public, &secret, id)
}

/// Send `record` one message carrying `id`, and what became of it: taken by its
/// distributor, or failed with what the distributor answered; and whether the distributor
/// answered that the endpoint is gone.
async fn woke(ctx: &Ctx, record: &Record, id: &str) -> (DeliveryOutcome, bool) {
    let Some(sealed) = sealed_for(record, id) else {
        let unreadable = DeliveryOutcome::Failed("its keys cannot be encrypted to".to_owned());
        return (unreadable, false);
    };
    let request = Request {
        method: Method::Post,
        url: record.endpoint.clone(),
        headers: vec![
            ("Content-Encoding".to_owned(), "aes128gcm".to_owned()),
            (
                "Content-Type".to_owned(),
                "application/octet-stream".to_owned(),
            ),
            ("TTL".to_owned(), HELD_FOR_SECONDS.to_owned()),
            ("Urgency".to_owned(), "high".to_owned()),
        ],
        body: None,
        pinned: None,
    };
    let sent = tokio::time::timeout(WAITED_FOR, ctx.seams.http.send_octets(&request, &sealed));
    match sent.await {
        Ok(Ok(answered)) if (200..300).contains(&answered.status) => (DeliveryOutcome::Sent, false),
        Ok(Ok(answered)) if GONE.contains(&answered.status) => (
            DeliveryOutcome::Failed(format!(
                "{}: its distributor no longer holds it, so the phone registers again",
                answered.status
            )),
            true,
        ),
        Ok(Ok(answered)) => (DeliveryOutcome::Failed(answered.status.to_string()), false),
        Ok(Err(why)) => (
            DeliveryOutcome::Failed(crate::error::withheld::without_credentials(
                &why.to_string(),
            )),
            false,
        ),
        Err(_) => (
            DeliveryOutcome::Failed("it did not answer in time".to_owned()),
            false,
        ),
    }
}

#[cfg(test)]
mod tests;
