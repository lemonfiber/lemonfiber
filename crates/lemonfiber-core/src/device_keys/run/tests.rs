use std::sync::Arc;

use lemonfiber_fixtures::attestations::ANDROID_RSA_ROOT;
use lemonfiber_fixtures::heard::Heard;
use lemonfiber_fixtures::ports::{Chance, Stopped};
use x509_parser::prelude::{FromDer as _, X509Certificate};

use super::{apps_changed, at, list, register, revoke, revoked_by, standing, swept, undone, used};
use crate::admission::Credential;
use crate::app::Ctx;
use crate::config::Settings;
use crate::device_keys::identity::{AppIdentity, SETTING};
use crate::device_keys::{encoded, Attestation, Backing, Kept, Unattested};
use crate::error::codes::device::{
    DEVICE_NAME_TAKEN, NOT_A_DEVICE_NAME, NOT_P256, NOT_REGISTERED, NO_ATTESTATION,
    PUBLIC_HALF_TAKEN, UNTRUSTED_APP,
};
use crate::journal::{Action, Kind, Undo};
use crate::test_support::a_context;

/// A machine keeping its configuration in a directory of this case's own, with a
/// password set, its clock at the moment the recorded chain is valid.
fn a_machine(named: &str) -> (Ctx, Arc<Heard>) {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("device-{named}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let heard = Arc::new(Heard::default());
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            stack_dir: Some(dir.join("stack")),
            admission: Some(dir.join("admission.json")),
            ..Settings::default()
        })
        .clock(Stopped::at(ANDROID_RSA_ROOT.at))
        .build()
        .with_random(Arc::new(Chance::cycling()))
        .with_narrator(heard.clone());
    password(&ctx, &lemonfiber_fixtures::passwords::a_password());
    (ctx, heard)
}

/// Set the operator's password to `chosen`.
fn password(ctx: &Ctx, chosen: &str) {
    let (Some(path), Ok(credential)) = (
        ctx.settings.admission.as_deref(),
        Credential::set(chosen, &Chance::cycling()),
    ) else {
        unreachable!("a password long enough is set");
    };
    assert!(crate::admission::keep(path, &credential).is_ok());
}

/// The recorded phone's attestation, and its public half as the phone sends it.
fn recorded() -> (Attestation, String) {
    let chain: Vec<String> = ANDROID_RSA_ROOT
        .chain
        .iter()
        .map(|der| encoded(der))
        .collect();
    let key = ANDROID_RSA_ROOT
        .chain
        .first()
        .and_then(|own| X509Certificate::from_der(own).ok())
        .map(|(_, own)| encoded(own.public_key().raw))
        .unwrap_or_default();
    (Attestation::Android { chain }, key)
}

/// The app the recorded phone's key was made by.
fn recorded_app(ctx: &Ctx) -> AppIdentity {
    let (attestation, key) = recorded();
    let Some(key) = crate::device_keys::PublicKey::read(&key) else {
        unreachable!("the recording carries a P-256 key");
    };
    match attestation.verified(&key, &[], ctx.seams.clock.now()) {
        Err(Unattested::Untrusted(named)) => AppIdentity::read(&named),
        _ => None,
    }
    .unwrap_or_else(|| unreachable!("the recording names its app"))
}

/// Trust `apps` as the operator's own builds.
fn trusting(ctx: &Ctx, apps: &[AppIdentity]) {
    let value: Vec<String> = apps.iter().map(AppIdentity::written).collect();
    let path =
        crate::app::targets::beside_env(ctx, crate::config::paths::SETTINGS).unwrap_or_default();
    let written = serde_json::json!({
        "schema": 1,
        "written_by": "0.0.0",
        "values": { SETTING: value.join(",") },
    });
    assert!(std::fs::write(path, written.to_string()).is_ok());
}

/// The recorded phone registered as `name` on a machine that trusts its app.
async fn registered(ctx: &Ctx, name: &str) {
    trusting(ctx, &[recorded_app(ctx)]);
    let (attestation, key) = recorded();
    assert!(register(ctx, name, &key, Some(&attestation)).await.is_ok());
}

/// The device keys the machine keeps, as they are on disk.
fn kept(ctx: &Ctx) -> Kept {
    at(ctx)
        .and_then(|path| crate::settings::kept::read(&path).ok())
        .unwrap_or_default()
}

/// The directory the machine keeps its configuration in.
fn config(ctx: &Ctx) -> std::path::PathBuf {
    crate::app::targets::layout(ctx)
        .map(|paths| paths.config_dir().to_path_buf())
        .unwrap_or_default()
}

/// The journal the machine keeps, as kinds.
fn journaled(ctx: &Ctx) -> Vec<Kind> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .map(|change| change.kind.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// The alerts the operator has been told, as their summaries.
fn told(ctx: &Ctx) -> Vec<String> {
    crate::app::outbox::load(ctx)
        .history()
        .into_iter()
        .map(|alert| alert.summary.clone())
        .collect()
}

#[tokio::test]
async fn a_recorded_phone_is_registered_with_its_backing_and_app_journaled_and_heard() {
    let (ctx, heard) = a_machine("registered");
    trusting(&ctx, &[recorded_app(&ctx)]);
    let (attestation, key) = recorded();
    let answered = register(&ctx, "pixel", &key, Some(&attestation)).await;
    let Ok(answered) = answered else {
        unreachable!("a trusted recording registers");
    };
    assert_eq!(answered.backing, Backing::Tee);
    assert_eq!(answered.app, recorded_app(&ctx).written());
    let record = kept(&ctx).active("pixel").cloned();
    assert_eq!(record.as_ref().map(|record| &record.public_key), Some(&key));
    assert_eq!(
        record.map(|record| record.fingerprint),
        Some(answered.fingerprint)
    );
    assert!(journaled(&ctx).contains(&Kind::DeviceKeyRegistered {
        name: "pixel".to_owned(),
        backing: "tee".to_owned()
    }));
    assert!(told(&ctx).iter().any(|said| said.contains("registered")));
    assert!(heard.said().iter().any(|said| said.contains("pixel")));
}

#[tokio::test]
async fn a_registration_is_refused_for_each_reason_with_a_code_of_its_own() {
    let (ctx, _) = a_machine("refused");
    let (attestation, key) = recorded();
    let code = |answered: Result<_, Box<crate::error::Problem>>| {
        answered.err().map(|problem| problem.code)
    };
    assert_eq!(
        code(register(&ctx, "Not A Name", &key, Some(&attestation)).await),
        Some(NOT_A_DEVICE_NAME)
    );
    assert_eq!(
        code(register(&ctx, "pixel", "AAAA", Some(&attestation)).await),
        Some(NOT_P256)
    );
    assert_eq!(
        code(register(&ctx, "pixel", &key, None).await),
        Some(NO_ATTESTATION)
    );
    assert_eq!(
        code(register(&ctx, "pixel", &key, Some(&attestation)).await),
        Some(UNTRUSTED_APP)
    );
    assert!(told(&ctx).iter().any(|said| said.contains("was refused")));
    assert!(kept(&ctx).keys.is_empty());
}

#[tokio::test]
async fn a_name_or_a_public_half_another_device_key_holds_is_refused_naming_it() {
    let (ctx, _) = a_machine("taken");
    registered(&ctx, "pixel").await;
    let (attestation, key) = recorded();
    let again = register(&ctx, "pixel", &key, Some(&attestation)).await;
    assert_eq!(
        again.err().map(|problem| problem.code),
        Some(DEVICE_NAME_TAKEN)
    );
    let twice = register(&ctx, "another", &key, Some(&attestation)).await;
    let Err(twice) = twice else {
        unreachable!("one public half registers once");
    };
    assert_eq!(twice.code, PUBLIC_HALF_TAKEN);
    assert!(twice.meaning.contains("pixel"));
}

#[tokio::test]
async fn a_revoke_is_journaled_heard_and_kept_in_the_listing_with_when() {
    let (ctx, _) = a_machine("revoked");
    registered(&ctx, "pixel").await;
    let Ok(listing) = revoke(&ctx, "pixel").await else {
        unreachable!("an active device key revokes");
    };
    assert_eq!(listing.revoked.as_deref(), Some("pixel"));
    assert!(listing.keys.iter().all(|key| key.revoked.is_some()));
    assert!(journaled(&ctx).contains(&Kind::DeviceKeyRevoked {
        name: "pixel".to_owned()
    }));
    assert!(told(&ctx).iter().any(|said| said.contains("was revoked")));
    let again = revoke(&ctx, "pixel").await;
    assert_eq!(
        again.err().map(|problem| problem.code),
        Some(NOT_REGISTERED)
    );
    assert_eq!(
        standing(&ctx, "pixel")
            .await
            .err()
            .map(|problem| problem.code),
        Some(NOT_REGISTERED)
    );
}

#[tokio::test]
async fn a_rehearsed_revoke_says_what_it_would_do_and_does_nothing() {
    let (ctx, _) = a_machine("rehearsed");
    registered(&ctx, "pixel").await;
    let rehearsing = Ctx {
        dry_run: true,
        ..ctx.clone()
    };
    let Ok(listing) = revoke(&rehearsing, "pixel").await else {
        unreachable!("a rehearsal of a revoke answers");
    };
    assert!(listing.rehearsed);
    assert!(kept(&ctx).active("pixel").is_some());
}

#[tokio::test]
async fn a_changed_password_revokes_every_device_key_at_the_next_look() {
    let (ctx, _) = a_machine("password");
    registered(&ctx, "pixel").await;
    password(&ctx, &lemonfiber_fixtures::passwords::another_password());
    let Ok(listing) = list(&ctx).await else {
        unreachable!("the listing reads");
    };
    assert!(listing.keys.iter().all(|key| key.revoked.is_some()));
    assert!(journaled(&ctx).contains(&Kind::DeviceKeyRevoked {
        name: "pixel".to_owned()
    }));
    assert!(told(&ctx).iter().any(|said| said.contains("was revoked")));
}

#[tokio::test]
async fn setting_the_password_revokes_at_once_and_so_does_removing_it() {
    let (ctx, _) = a_machine("set");
    registered(&ctx, "pixel").await;
    password(&ctx, &lemonfiber_fixtures::passwords::another_password());
    assert!(swept(&ctx).await.is_ok());
    assert!(kept(&ctx).active("pixel").is_none());
    let (ctx, _) = a_machine("removed");
    registered(&ctx, "pixel").await;
    if let Some(path) = ctx.settings.admission.as_deref() {
        assert!(crate::admission::forget(path).is_ok());
    }
    assert!(swept(&ctx).await.is_ok());
    assert!(kept(&ctx).active("pixel").is_none());
}

#[tokio::test]
async fn removing_an_app_revokes_every_device_key_it_made_and_no_other() {
    let (ctx, _) = a_machine("apps");
    registered(&ctx, "pixel").await;
    let other: Vec<AppIdentity> = AppIdentity::read("ios:ABCDE12345.app.other")
        .into_iter()
        .collect();
    trusting(&ctx, &[other.clone(), vec![recorded_app(&ctx)]].concat());
    assert!(swept(&ctx).await.is_ok());
    assert!(kept(&ctx).active("pixel").is_some());
    trusting(&ctx, &other);
    assert!(standing(&ctx, "pixel").await.is_err());
    assert!(kept(&ctx).active("pixel").is_none());
    assert!(journaled(&ctx).contains(&Kind::DeviceKeyRevoked {
        name: "pixel".to_owned()
    }));
    assert!(told(&ctx)
        .iter()
        .any(|said| said.contains("The device key named pixel was revoked")));
}

#[tokio::test]
async fn settings_that_cannot_be_read_revoke_nothing_and_leave_nothing_standing() {
    let (ctx, _) = a_machine("unreadable");
    registered(&ctx, "pixel").await;
    let settings =
        crate::app::targets::beside_env(&ctx, crate::config::paths::SETTINGS).unwrap_or_default();
    assert!(std::fs::write(&settings, "not a settings store").is_ok());
    assert!(standing(&ctx, "pixel").await.is_err());
    assert!(kept(&ctx).active("pixel").is_some());
    assert!(revoke(&ctx, "pixel").await.is_ok());
    assert!(kept(&ctx).active("pixel").is_none());
}

#[tokio::test]
async fn a_journal_that_cannot_be_written_still_leaves_every_revoke_made_and_told() {
    let (ctx, _) = a_machine("unjournaled");
    registered(&ctx, "pixel").await;
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    assert!(std::fs::remove_file(&journal).is_ok());
    assert!(std::fs::create_dir_all(&journal).is_ok());
    assert!(revoke(&ctx, "pixel").await.is_err());
    assert!(kept(&ctx).active("pixel").is_none());
    assert!(told(&ctx)
        .iter()
        .any(|said| said.contains("The device key named pixel was revoked")));
}

#[tokio::test]
async fn a_sweep_whose_journal_cannot_be_written_revokes_every_stale_key_all_the_same() {
    let (ctx, _) = a_machine("unjournaled-sweep");
    registered(&ctx, "pixel").await;
    password(&ctx, &lemonfiber_fixtures::passwords::another_password());
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    assert!(std::fs::remove_file(&journal).is_ok());
    assert!(std::fs::create_dir_all(&journal).is_ok());
    assert!(standing(&ctx, "pixel").await.is_err());
    assert!(kept(&ctx).active("pixel").is_none());
}

#[tokio::test]
async fn a_proved_mint_is_written_down_as_its_last_use() {
    let (ctx, _) = a_machine("used");
    registered(&ctx, "pixel").await;
    assert!(used(&ctx, "pixel").await.is_ok());
    assert!(kept(&ctx)
        .active("pixel")
        .is_some_and(|record| record.used.is_some()));
    assert_eq!(
        used(&ctx, "nobody").await.err().map(|problem| problem.code),
        Some(NOT_REGISTERED)
    );
}

#[tokio::test]
async fn undoing_a_registration_revokes_it_and_says_which() {
    let (ctx, _) = a_machine("undone");
    registered(&ctx, "pixel").await;
    let undo = Undo {
        target: "pixel".to_owned(),
        action: Action::RevokeDevice {
            name: "pixel".to_owned(),
        },
    };
    assert_eq!(revoked_by(&undo), Some("pixel"));
    assert!(undone(&ctx, "pixel").await.is_ok());
    assert!(kept(&ctx).active("pixel").is_none());
    assert!(undone(&ctx, "pixel").await.is_ok());
    let other = Undo {
        target: "pixel".to_owned(),
        action: Action::Revoke {
            name: "pixel".to_owned(),
        },
    };
    assert_eq!(revoked_by(&other), None);
}

#[tokio::test]
async fn a_restore_revokes_every_device_key_and_says_why() {
    let (ctx, _) = a_machine("restored");
    registered(&ctx, "pixel").await;
    assert!(super::restored(&ctx, &config(&ctx)).await.is_ok());
    assert!(kept(&ctx).active("pixel").is_none());
    assert!(journaled(&ctx).contains(&Kind::DeviceKeyRevoked {
        name: "pixel".to_owned()
    }));
    assert!(crate::app::outbox::load(&ctx)
        .history()
        .into_iter()
        .any(|alert| alert.meaning.contains("backup was restored")));
}

#[tokio::test]
async fn a_revoked_device_keys_alert_names_the_keys_it_proved_which_stay() {
    let (ctx, _) = a_machine("proved");
    registered(&ctx, "pixel").await;
    let minted = crate::keys::run::mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &crate::keys::Minter::Operator,
        Some("pixel"),
    )
    .await;
    assert!(minted.is_ok());
    assert!(revoke(&ctx, "pixel").await.is_ok());
    let outbox = crate::app::outbox::load(&ctx);
    let alerts = outbox.history();
    let revoked = alerts.iter().find(|alert| {
        alert
            .summary
            .contains("The device key named pixel was revoked")
    });
    assert!(
        revoked.is_some_and(|alert| alert.meaning.contains("the keys ha")
            && alert
                .remedies
                .iter()
                .any(|remedy| remedy.contains("key revoke ha")))
    );
    let keys = crate::keys::run::at(&ctx)
        .and_then(|path| crate::keys::Kept::at(&path).ok())
        .unwrap_or_default();
    assert_eq!(keys.proved_with("pixel").len(), 1);
}

#[tokio::test]
async fn changing_the_apps_names_each_identity_gained_or_lost_and_revokes_what_a_lost_one_made() {
    let (ctx, _) = a_machine("apps-changed");
    registered(&ctx, "pixel").await;
    let recorded = recorded_app(&ctx).written();
    let other = "ios:ABCDE12345.app.other";
    let both = format!("{recorded},{other}");
    assert!(apps_changed(&ctx, Some(&recorded), Some(&both))
        .await
        .is_ok());
    assert!(told(&ctx)
        .iter()
        .any(|said| said == &format!("The app {other} may now register device keys")));
    assert!(kept(&ctx).active("pixel").is_some());
    trusting(
        &ctx,
        &AppIdentity::read(other).into_iter().collect::<Vec<_>>(),
    );
    assert!(apps_changed(&ctx, Some(&both), Some(other)).await.is_ok());
    assert!(told(&ctx)
        .iter()
        .any(|said| said == &format!("The app {recorded} may no longer register device keys")));
    assert!(kept(&ctx).active("pixel").is_none());
}

#[tokio::test]
async fn putting_back_the_run_that_registered_a_device_key_revokes_it_and_records_the_revoke() {
    let (ctx, heard) = a_machine("put-back");
    registered(&ctx, "pixel").await;
    let Some(at) = crate::app::targets::layout(&ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .and_then(|journal| journal.changes().last().map(|change| change.at.clone()))
    else {
        unreachable!("a registration is journaled")
    };
    let mut today = ctx.clone();
    today.seams.clock = a_context().build().seams.clock;
    assert!(crate::app::putting_back::undo(&today, Some(&at))
        .await
        .is_ok());
    assert!(kept(&ctx).active("pixel").is_none());
    assert!(journaled(&ctx).contains(&Kind::DeviceKeyRevoked {
        name: "pixel".to_owned()
    }));
    assert!(heard
        .said()
        .iter()
        .any(|said| said.contains("The device key named pixel was revoked")));
}

#[tokio::test]
async fn a_registration_whose_journal_cannot_be_written_registers_nothing() {
    let (ctx, _) = a_machine("unjournaled-registration");
    trusting(&ctx, &[recorded_app(&ctx)]);
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    let _ = std::fs::remove_file(&journal);
    assert!(std::fs::create_dir_all(&journal).is_ok());
    let (attestation, key) = recorded();
    assert!(register(&ctx, "pixel", &key, Some(&attestation))
        .await
        .is_err());
    assert!(kept(&ctx).named("pixel").is_none());
}

#[test]
fn each_reason_an_attestation_vouches_for_nothing_is_refused_under_a_code_of_its_own() {
    use crate::error::codes::device::{ATTESTATION_REVOKED, ATTESTATION_UNVERIFIED, IN_SOFTWARE};
    let unproved = |why: Unattested| super::refused::unproved(&why).code;
    assert_eq!(unproved(Unattested::Unverified), ATTESTATION_UNVERIFIED);
    assert_eq!(unproved(Unattested::InSoftware), IN_SOFTWARE);
    assert_eq!(unproved(Unattested::Revoked), ATTESTATION_REVOKED);
    assert_eq!(
        unproved(Unattested::Untrusted("ios:ABCDE12345.app.other".to_owned())),
        UNTRUSTED_APP
    );
    assert_eq!(
        super::refused::no_challenge().code,
        crate::error::codes::device::NO_CHALLENGE
    );
}

#[tokio::test]
async fn a_machine_keeping_no_configuration_registers_and_lists_no_device_key() {
    let ctx = a_context().build();
    let (attestation, key) = recorded();
    let registered = register(&ctx, "pixel", &key, Some(&attestation)).await;
    assert_eq!(
        registered.err().map(|problem| problem.code),
        Some(super::refused::nowhere().code)
    );
    assert!(list(&ctx).await.is_err());
}
