use axum::http::{header, HeaderMap, HeaderValue};
use lemonfiber_api::admission::here;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::certificate;
use lemonfiber_core::companion::{paired, served};
use lemonfiber_core::config::Settings;
use lemonfiber_core::error::codes::serve::{NO_CERTIFICATE, UNSETTLED_PORT};
use lemonfiber_core::platform::Environment;
use lemonfiber_fixtures::ports::Renamed;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_testing::context::a_context;

use super::{bound, encrypting};

/// A machine keeping what pairing needs at `directory`.
fn keeping(directory: Option<std::path::PathBuf>) -> Ctx {
    a_context()
        .settings(Settings {
            companion: directory,
            ..Settings::default()
        })
        .build()
}

/// Not asked for, nothing is made and nothing is presented.
#[test]
fn a_run_not_asked_to_encrypt_presents_nothing() {
    let at = Scratch::new("encrypting-unasked").kept();
    assert!(matches!(
        encrypting(&keeping(Some(at.clone())), false, None),
        Ok(None)
    ));
    assert_eq!(certificate::kept(&at), Ok(None), "and nothing is made");
}

/// Asked for, the certificate is made once and presented with the fingerprint a phone
/// pins, and a later run presents the same one.
#[test]
fn a_run_asked_to_encrypt_presents_the_certificate_it_keeps() {
    let at = Scratch::new("encrypting-asked").kept();
    let first = encrypting(&keeping(Some(at.clone())), true, Some(8443))
        .ok()
        .flatten()
        .map(|on| on.fingerprint);
    let second = encrypting(&keeping(Some(at.clone())), true, Some(8443))
        .ok()
        .flatten()
        .map(|on| on.fingerprint);
    assert!(first.is_some());
    assert_eq!(first, second);
    assert_eq!(
        certificate::kept(&at)
            .ok()
            .flatten()
            .map(|held| held.fingerprint),
        first
    );
}

/// Each way of not being able to encrypt is refused by name.
#[test]
fn a_run_that_cannot_encrypt_is_refused_by_name() {
    let code = |ctx: &Ctx, port| {
        encrypting(ctx, true, port)
            .err()
            .map(|problem| problem.code)
    };
    let at = Scratch::new("encrypting-refused").kept();
    assert_eq!(code(&keeping(Some(at.clone())), None), Some(UNSETTLED_PORT));
    assert_eq!(code(&keeping(None), Some(8443)), Some(NO_CERTIFICATE));

    let _ = std::fs::write(at.join("key.pem"), "half a pair");
    assert_eq!(
        code(&keeping(Some(at.clone())), Some(8443)),
        Some(NO_CERTIFICATE)
    );

    let _ = certificate::replaced(&at);
    let other = Scratch::new("encrypting-another-key").kept();
    let _ = certificate::replaced(&other);
    let _ = std::fs::copy(other.join("key.pem"), at.join("key.pem"));
    assert_eq!(
        code(&keeping(Some(at)), Some(8443)),
        Some(NO_CERTIFICATE),
        "a key that is not the certificate's own does not serve"
    );
}

/// A machine called `den`, served encrypted on the network on `port`, keeping what
/// pairing needs.
fn served_as_den(named: &str, port: u16) -> Ctx {
    let at = Scratch::new(named).kept();
    let _ = certificate::kept_or_made(&at);
    let _ = served::record(
        &at,
        served::Served {
            port,
            encrypted: true,
            network: true,
        },
    );
    a_context()
        .settings(Settings {
            companion: Some(at),
            ..Settings::default()
        })
        .environment(Environment::MacOs)
        .build()
        .with_site(Renamed::called(Some("den")))
}

/// A request naming `host`, as a phone that was handed it sends one.
fn naming(host: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(host) {
        headers.insert(header::HOST, value);
    }
    headers
}

/// The address pairing hands a phone is one the run it was made for answers: a phone
/// that was paired reaches the surface by exactly that name, so a run refusing it is a
/// pairing that cannot be used.
#[tokio::test]
async fn the_address_a_phone_is_handed_is_one_the_run_answers() {
    let ctx = served_as_den("encrypting-answers-the-paired-name", 8443);
    let address = paired(&ctx)
        .await
        .map(|made| made.material.address)
        .unwrap_or_default();
    let host = address.strip_prefix("https://").unwrap_or_default();
    assert_eq!(host, "den.local:8443");

    assert!(here(&naming(host), &bound(&ctx, 8443, true, true).await));
    // Only where it is served encrypted on the network, which is the only run
    // pairing is made for. Anywhere else a name is refused.
    assert!(!here(&naming(host), &bound(&ctx, 8443, true, false).await));
    assert!(!here(&naming(host), &bound(&ctx, 8443, false, true).await));
    assert!(!here(
        &naming("elsewhere.local:8443"),
        &bound(&ctx, 8443, true, true).await
    ));
}
