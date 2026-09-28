use lemonfiber_core::app::Ctx;
use lemonfiber_core::companion::certificate;
use lemonfiber_core::config::Settings;
use lemonfiber_core::error::codes::serve::{NO_CERTIFICATE, UNSETTLED_PORT};
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_testing::context::a_context;

use super::encrypting;

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
