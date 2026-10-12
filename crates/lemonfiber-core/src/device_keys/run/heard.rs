//! What the operator is told about a device key: its registration, its revoke, a
//! registration refused and a signature that did not verify.

use crate::alert::{Alert, Moment};
use crate::app::Ctx;
use crate::error::{Problem, Severity};
use crate::PRODUCT;

use super::super::Record;

/// What the alerts about device keys are filed under, ahead of the device key's name.
const CHECK: &str = "device-key.";

/// The kind every alert about a device key shares.
const ALERT_KIND: &str = "device-key";

/// Why a device key was revoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Revoked {
    /// Somebody asked.
    Asked,
    /// The password it was registered under changed.
    PasswordChanged,
    /// The app that made it was taken off the apps this stack trusts.
    AppRemoved,
}

/// What a device key's alert is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Heard {
    /// It was registered.
    Registered,
    /// It was revoked.
    Revoked(Revoked),
    /// A mint carried a signature it did not make.
    Misproved,
}

/// Tell the operator, now and in the record of alerts, what became of a device key.
pub(super) async fn heard(ctx: &Ctx, record: &Record, heard: &Heard) {
    let check = format!("{CHECK}{}", record.name);
    let (summary, meaning, remedy) = said(record, heard);
    let happened = match heard {
        Heard::Registered => "registered".to_owned(),
        Heard::Revoked(_) => "revoked".to_owned(),
        Heard::Misproved => format!("misproved-{}", ctx.stamp()),
    };
    raised(ctx, &check, &happened, summary, meaning, remedy).await;
}

/// Tell the operator that registering a device key under `name` was refused, and why.
pub(super) async fn refused(ctx: &Ctx, name: &str, why: &Problem) {
    let check = format!("{CHECK}{name}");
    raised(
        ctx,
        &check,
        &format!("refused-{}", ctx.stamp()),
        format!("Registering a device key named {name} was refused"),
        format!(
            "Somebody who gave the operator password asked to register a phone's key, and it \
             was refused: {}",
            why.summary
        ),
        "If nobody here asked, change the password: whoever asked knows it".to_owned(),
    )
    .await;
}

/// The sentences an alert about `record` says.
fn said(record: &Record, heard: &Heard) -> (String, String, String) {
    let name = &record.name;
    match heard {
        Heard::Registered => (
            format!(
                "A device key named {name} was registered, held in {}",
                record.backing.written()
            ),
            format!(
                "The phone holding it can prove the operator for a mint from now until it is \
                 revoked. It was made by {}.",
                record.app
            ),
            "If nobody here registered it, revoke it and change the password".to_owned(),
        ),
        Heard::Revoked(why) => (
            format!("The device key named {name} was revoked"),
            match why {
                Revoked::Asked => "The phone holding it proves nothing from now on.".to_owned(),
                Revoked::PasswordChanged => "The password it was registered under changed, so \
                     the phone holding it proves nothing from now on."
                    .to_owned(),
                Revoked::AppRemoved => format!(
                    "The app that made it, {}, is no longer one this stack trusts.",
                    record.app
                ),
            },
            "Register the phone's key again if it should still prove the operator".to_owned(),
        ),
        Heard::Misproved => (
            format!("A mint carried a signature the device key named {name} did not make"),
            format!(
                "Somebody holding an operator session asked {PRODUCT} for a key and could not \
                 prove it with that phone. Nothing was minted."
            ),
            "If nobody here asked, change the password".to_owned(),
        ),
    }
}

/// Raise one alert about a device key, filed under `check` for what `happened`.
///
/// Recorded as delivered once said, because the narrator cannot fail.
async fn raised(
    ctx: &Ctx,
    check: &str,
    happened: &str,
    summary: String,
    meaning: String,
    remedy: String,
) {
    let alert = Alert {
        id: Some(Alert::named(check, happened)),
        check: check.to_owned(),
        kind: ALERT_KIND.to_owned(),
        moment: Moment::Onset,
        severity: Severity::Warning,
        summary,
        meaning,
        remedies: vec![remedy],
        affected: vec![check.to_owned()],
        exit: None,
    };
    let mut outbox = crate::app::outbox::load(ctx);
    outbox.owe(vec![alert.clone()]);
    ctx.narrator.say(&alert.summary).await;
    outbox.delivered(&|_| 0);
    crate::app::outbox::save(ctx, &outbox);
}
