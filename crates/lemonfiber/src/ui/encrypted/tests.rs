use axum::http::{header, HeaderMap, HeaderValue};
use lemonfiber_api::admission::here;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::certificate;
use lemonfiber_core::companion::{paired, served};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_error::codes::serve::{NO_CERTIFICATE, UNSETTLED_PORT};
use lemonfiber_fixtures::ports::Renamed;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_testing::context::a_context;

use lemonfiber_core::app::{dispatch, Command, Outcome};
use lemonfiber_core::companion::served::ServedReport;
use lemonfiber_error::Problem;

use super::{bound, encrypting, port_settled, Encrypting};

/// How a run on `port` asked whether to encrypt and whether to offer the network is
/// served, as the serving loop settles it.
async fn settled(
    ctx: &Ctx,
    port: u16,
    encrypted: bool,
    network: bool,
) -> Result<ServedReport, Box<Problem>> {
    let asked = Command::Served(served::Served {
        port,
        encrypted,
        network,
    });
    match dispatch(asked, ctx).await? {
        Outcome::Served(report) => Ok(report),
        _ => Ok(ServedReport::default()),
    }
}

/// What a run asked whether to encrypt on `port` presents.
async fn presenting(
    ctx: &Ctx,
    tls: bool,
    port: Option<u16>,
) -> Result<Option<Encrypting>, Box<Problem>> {
    port_settled(tls, port)?;
    let report = settled(ctx, port.unwrap_or_default(), tls, false).await?;
    encrypting(&report)
}

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
#[tokio::test]
async fn a_run_not_asked_to_encrypt_presents_nothing() {
    let at_scratch = Scratch::new("encrypting-unasked");
    let at = at_scratch.to_path_buf();
    assert!(matches!(
        presenting(&keeping(Some(at.clone())), false, None).await,
        Ok(None)
    ));
    assert_eq!(
        certificate::kept(&lemonfiber_adapters::Disk, &at),
        Ok(None),
        "and nothing is made"
    );
}

/// Asked for, the certificate is made once and presented with the fingerprint a phone
/// pins, and a later run presents the same one.
#[tokio::test]
async fn a_run_asked_to_encrypt_presents_the_certificate_it_keeps() {
    let at_scratch = Scratch::new("encrypting-asked");
    let at = at_scratch.to_path_buf();
    let first = presenting(&keeping(Some(at.clone())), true, Some(8443))
        .await
        .ok()
        .flatten()
        .map(|on| on.fingerprint);
    let second = presenting(&keeping(Some(at.clone())), true, Some(8443))
        .await
        .ok()
        .flatten()
        .map(|on| on.fingerprint);
    assert!(first.is_some());
    assert_eq!(first, second);
    assert_eq!(
        certificate::kept(&lemonfiber_adapters::Disk, &at)
            .ok()
            .flatten()
            .map(|held| held.fingerprint),
        first
    );
}

/// Each way of not being able to encrypt is refused by name.
#[tokio::test]
async fn a_run_that_cannot_encrypt_is_refused_by_name() {
    async fn code(ctx: &Ctx, port: Option<u16>) -> Option<lemonfiber_error::Code> {
        presenting(ctx, true, port)
            .await
            .err()
            .map(|problem| problem.code)
    }
    let at_scratch = Scratch::new("encrypting-refused");
    let at = at_scratch.to_path_buf();
    assert_eq!(
        code(&keeping(Some(at.clone())), None).await,
        Some(UNSETTLED_PORT)
    );
    assert_eq!(code(&keeping(None), Some(8443)).await, Some(NO_CERTIFICATE));

    let _ = std::fs::write(at.join("key.pem"), "half a pair");
    assert_eq!(
        code(&keeping(Some(at.clone())), Some(8443)).await,
        Some(NO_CERTIFICATE)
    );

    let _ = certificate::replaced(&lemonfiber_adapters::Disk, &at);
    let other_scratch = Scratch::new("encrypting-another-key");
    let other = other_scratch.to_path_buf();
    let _ = certificate::replaced(&lemonfiber_adapters::Disk, &other);
    let _ = std::fs::copy(other.join("key.pem"), at.join("key.pem"));
    assert_eq!(
        code(&keeping(Some(at)), Some(8443)).await,
        Some(NO_CERTIFICATE),
        "a key that is not the certificate's own does not serve"
    );
}

/// A machine called `den`, served encrypted on the network on `port`, keeping what
/// pairing needs.
fn served_as_den(named: &str, port: u16) -> Ctx {
    let at_scratch = Scratch::new(named);
    let at = at_scratch.to_path_buf();
    let _ = certificate::kept_or_made(&lemonfiber_adapters::Disk, &at);
    let _ = served::record(
        &at,
        served::Served {
            port,
            encrypted: true,
            network: true,
        },
    );
    a_context()
        .holding(at_scratch)
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

    let binding = |network: bool, encrypted: bool| {
        let ctx = &ctx;
        async move {
            let report = settled(ctx, 8443, encrypted, network)
                .await
                .unwrap_or_default();
            bound(&report, 8443, network)
        }
    };
    assert!(here(&naming(host), &binding(true, true).await));
    assert!(!here(&naming(host), &binding(true, false).await));
    assert!(!here(&naming(host), &binding(false, true).await));
    assert!(!here(
        &naming("elsewhere.local:8443"),
        &binding(true, true).await
    ));
}
