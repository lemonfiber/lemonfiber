//! A client that reaches a service only over TLS, and only where the certificate it
//! presents is one whose SHA-256 the client was handed.
//!
//! A certificate the core makes for a service inside the stack is signed by no authority,
//! so it is judged by its SHA-256 alone: SHA-256 over the end-entity certificate's DER
//! encoding, in lower-case hexadecimal, the form every pin lemonfiber hands out is written
//! in. A certificate whose SHA-256 is not among the pins is refused before anything is
//! sent, and every signature the handshake makes with an accepted one is still checked.
//! Nothing else is trusted: no public root, no proxy named in the environment, and no
//! redirect to anywhere else.

use std::sync::Arc;
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, SignatureScheme};

#[cfg(any(test, feature = "test-support"))]
pub mod fake;

/// How many lower-case hexadecimal digits a certificate's SHA-256 is written in.
const DIGITS: usize = 64;

/// The SHA-256 of `der`, a certificate's DER encoding, in lower-case hexadecimal: the form
/// a pin is written in.
#[must_use]
pub fn sha256(der: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, der)
        .as_ref()
        .iter()
        .fold(String::with_capacity(DIGITS), |mut out, byte| {
            use std::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// A verifier accepting only a certificate among `pins`, and checking every signature the
/// handshake makes with it with the provider's own algorithms.
#[derive(Debug)]
struct Pinned {
    /// The SHA-256 of each certificate accepted.
    pins: Vec<String>,
    /// Whose algorithms check the handshake's signatures.
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if self.pins.contains(&sha256(end_entity.as_ref())) {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// A client reaching only HTTPS, accepting only a certificate among `pins`, following no
/// redirect, taking no proxy from the environment, and giving the server `within` to
/// answer where it is given a bound; nothing where no certificate is pinned, so nothing is
/// reachable, or where TLS could not be set up.
#[must_use]
pub fn client(pins: &[String], within: Option<Duration>) -> Option<reqwest::Client> {
    if pins.is_empty() {
        return None;
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .ok()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned {
            pins: pins.to_vec(),
            provider,
        }))
        .with_no_client_auth();
    let builder = reqwest::Client::builder()
        .use_preconfigured_tls(config)
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy();
    match within {
        Some(within) => builder.timeout(within),
        None => builder,
    }
    .build()
    .ok()
}

/// Whether a handshake to `base`, an `https://host:port` address, presents exactly the
/// certificate whose SHA-256 is `pin`, the server given `within` to answer.
///
/// Any answer the server makes over that handshake counts, whatever its status: the
/// question is what it presents, not what it serves. An address not reached over HTTPS
/// presents nothing, and nothing is sent to it.
pub async fn presents(base: &str, pin: &str, within: Duration) -> bool {
    let asked = https(base)
        .zip(client(&[pin.to_owned()], Some(within)))
        .map(|(base, client)| client.head(base).send());
    futures_util::future::OptionFuture::from(asked)
        .await
        .is_some_and(|answered| answered.is_ok())
}

/// `text` as an address, where it is one reached over HTTPS.
fn https(text: &str) -> Option<reqwest::Url> {
    reqwest::Url::parse(text)
        .ok()
        .filter(|url| url.scheme() == "https")
}

#[cfg(test)]
mod tests;
