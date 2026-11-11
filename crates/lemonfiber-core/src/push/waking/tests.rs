//! A wake that cannot be drawn, sealed or carried is a failure, never a message sent.

use std::path::PathBuf;
use std::sync::Arc;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Chance;
use ring::agreement::{EphemeralPrivateKey, ECDH_P256};
use ring::rand::SystemRandom;

use super::{delivered, woke};
use crate::app::Ctx;
use crate::config::Settings;
use crate::model::DeliveryOutcome;
use crate::ports::http::Http;
use crate::push::{Kept, Record};

fn record(auth: &str) -> Record {
    Record {
        name: "pixel".to_owned(),
        endpoint: "https://push.example/a".to_owned(),
        p256dh: EphemeralPrivateKey::generate(&ECDH_P256, &SystemRandom::new())
            .and_then(|key| key.compute_public_key())
            .map(|public| URL_SAFE_NO_PAD.encode(public.as_ref()))
            .unwrap_or_default(),
        auth: auth.to_owned(),
        registered: String::new(),
        owed: 1,
        owner: None,
        wakes: Vec::new(),
        last: None,
        failures: Vec::new(),
        proven: None,
    }
}

fn readable() -> Record {
    record(&URL_SAFE_NO_PAD.encode([9_u8; 16]))
}

fn reaching(http: &Arc<Fake>) -> Ctx {
    crate::test_support::a_context()
        .build()
        .with_http(Arc::clone(http) as Arc<dyn Http>)
}

fn failed(outcome: &(DeliveryOutcome, bool)) -> bool {
    matches!(outcome, (DeliveryOutcome::Failed(_), false))
}

#[tokio::test]
async fn a_phone_whose_secret_cannot_be_read_is_sent_nothing() {
    let http = Fake::always(Answer::reply(201, ""));

    let outcome = woke(&reaching(&http), &record("not base64url!"), "w1").await;

    assert!(failed(&outcome), "{outcome:?}");
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn a_distributor_that_cannot_be_reached_is_a_failure_and_not_gone() {
    let http = Fake::silent();

    let outcome = woke(&reaching(&http), &readable(), "w1").await;

    assert!(failed(&outcome), "{outcome:?}");
    assert_eq!(http.requests().len(), 1);
}

#[tokio::test]
async fn nothing_is_sent_where_no_id_can_be_drawn_for_a_wake() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("push-undrawn").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    let http = Fake::always(Answer::reply(201, ""));
    let ctx = crate::test_support::a_context()
        .settings(Settings {
            env_file: Some(dir.join("config").join(".env")),
            ..Settings::default()
        })
        .build()
        .with_http(Arc::clone(&http) as Arc<dyn Http>)
        .with_random(Arc::new(Chance::exactly(None)));
    let path: PathBuf = crate::push::run::kept_at(&ctx).unwrap_or_default();
    let kept = crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
        kept.endpoints = vec![readable()];
        Ok(())
    });
    assert!(kept.is_ok());

    delivered(&ctx, &path, "pixel").await;

    assert!(http.requests().is_empty());
    let wakes = crate::push::run::read_kept(&path)
        .ok()
        .and_then(|kept| kept.endpoints.into_iter().next())
        .map(|one| one.wakes);
    assert_eq!(wakes, Some(Vec::new()));
}
