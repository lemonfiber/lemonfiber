//! What the operator is told about a device key: its registration, its revoke, a
//! registration refused, a signature that did not verify, and an app the operator added
//! to or removed from the ones that may register one.

use crate::alert::{Alert, Moment};
use crate::app::Ctx;
use crate::error::{Problem, Severity};
use crate::PRODUCT;

use super::super::identity::SETTING;
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
    /// A configuration backup was restored, putting back a password and device keys
    /// from before whatever was revoked since.
    Restored,
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
///
/// A revoke names every key the device key proved that still stands, since those stay.
pub(super) async fn heard(ctx: &Ctx, record: &Record, heard: &Heard) {
    let check = format!("{CHECK}{}", record.name);
    let (summary, mut meaning, mut remedy) = said(record, heard);
    if matches!(heard, Heard::Revoked(_)) {
        let proved = proved(ctx, &record.name);
        if !proved.is_empty() {
            meaning = format!(
                "{meaning} It proved the mint of the keys {}, which still stand.",
                proved.join(", ")
            );
            remedy = format!(
                "If those keys should go too, revoke them: {}",
                proved
                    .iter()
                    .map(|name| format!("{PRODUCT} key revoke {name}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
    }
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

/// Tell the operator that the app `identity` was added to the ones that may register a
/// device key, where `added`, or taken off them.
pub(super) async fn app(ctx: &Ctx, identity: &str, added: bool) {
    let (summary, meaning) = if added {
        (
            format!("The app {identity} may now register device keys"),
            "A phone running it can register a key with the operator password, and that key \
             can then prove the operator for a mint."
                .to_owned(),
        )
    } else {
        (
            format!("The app {identity} may no longer register device keys"),
            "Every device key it registered was revoked.".to_owned(),
        )
    };
    raised(
        ctx,
        &format!("{CHECK}app.{identity}"),
        &format!(
            "{}-{}",
            if added { "added" } else { "removed" },
            ctx.stamp()
        ),
        summary,
        meaning,
        format!("If nobody here changed it, look at {SETTING} in {PRODUCT} settings"),
    )
    .await;
}

/// The names of every key the device key `device` proved that still stands.
fn proved(ctx: &Ctx, device: &str) -> Vec<String> {
    crate::keys::run::at(ctx)
        .and_then(|path| crate::keys::Kept::at(&path).ok())
        .map(|kept| {
            kept.proved_with(device)
                .into_iter()
                .map(|record| record.name.clone())
                .collect()
        })
        .unwrap_or_default()
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
                Revoked::Restored => "A backup was restored, which puts back a password and \
                     device keys from before anything revoked since, so every phone registers \
                     again."
                    .to_owned(),
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
