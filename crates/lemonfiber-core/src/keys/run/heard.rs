//! What the operator is told about a key: its mint, its collection, its revoke and the
//! lapse of a claim nobody collected.

use crate::alert::{Alert, Moment};
use crate::app::Ctx;
use crate::error::Severity;
use crate::PRODUCT;

use super::super::Record;
use super::{ALERT_KIND, CHECK};

/// What a key's alert is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Heard {
    /// It was minted.
    Minted,
    /// It was collected with its claim, from the address named where it is known.
    Collected {
        /// The address it was collected from.
        from: Option<String>,
    },
    /// It was revoked.
    Revoked,
    /// Its claim lapsed with nobody having collected it, so it was revoked.
    Lapsed,
}

/// Tell the operator, now and in the record of alerts, what became of a key.
///
/// Recorded as delivered once said, because the narrator cannot fail.
pub(super) async fn heard(ctx: &Ctx, record: &Record, heard: Heard) {
    let alert = alert(record, &heard);
    let mut outbox = crate::app::outbox::load(ctx);
    outbox.owe(vec![alert.clone()]);
    ctx.narrator.say(&alert.summary).await;
    outbox.delivered(&|_| 0);
    crate::app::outbox::save(ctx, &outbox);
}

/// The alert a key raises, naming the key and its scope.
///
/// A key is minted, collected and revoked at most once each, and a name is never minted
/// again, so which of those happened is all that tells its alerts apart.
fn alert(record: &Record, heard: &Heard) -> Alert {
    let scope = record.scope.written();
    let (summary, meaning, remedy) = match heard {
        Heard::Minted => (
            format!(
                "A key named {} was minted, with the scope {scope}",
                record.name
            ),
            format!(
                "Whoever holds it can reach {PRODUCT} as far as {scope} admits, from now until \
                 it is revoked."
            ),
            format!(
                "If nobody meant to mint it, revoke it: {PRODUCT} key revoke {}",
                record.name
            ),
        ),
        Heard::Collected { from } => (
            format!(
                "The key named {} was collected, with the scope {scope}, for {}",
                record.name,
                record.purpose.written()
            ),
            format!(
                "Whatever collected it holds the key now, from {}, as far as {scope} admits, \
                 until it is revoked.",
                from.as_deref()
                    .unwrap_or("an address that could not be told")
            ),
            format!(
                "If nothing you meant collected it, revoke it: {PRODUCT} key revoke {}",
                record.name
            ),
        ),
        Heard::Revoked => (
            format!(
                "The key named {} was revoked, with the scope {scope}",
                record.name
            ),
            "Whatever held it is refused from its next request.".to_owned(),
            "Mint a new key for anything that still needs one".to_owned(),
        ),
        Heard::Lapsed => (
            format!(
                "The key named {} was revoked, because nothing collected it in time",
                record.name
            ),
            "Its claim stopped collecting anything, so the key never had a secret and nothing \
             holds it."
                .to_owned(),
            "Mint it as a claim again, and collect it within ten minutes".to_owned(),
        ),
    };
    let check = format!("{CHECK}{}", record.name);
    let happened = match heard {
        Heard::Minted => "minted",
        Heard::Collected { .. } => "collected",
        Heard::Revoked | Heard::Lapsed => "revoked",
    };
    Alert {
        id: Some(Alert::named(&check, happened)),
        check: check.clone(),
        kind: ALERT_KIND.to_owned(),
        moment: Moment::Onset,
        severity: Severity::Warning,
        summary,
        meaning,
        remedies: vec![remedy],
        affected: vec![check],
        exit: None,
    }
}
