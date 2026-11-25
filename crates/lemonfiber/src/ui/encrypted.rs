//! Serving the web surface encrypted, when it is asked to be.
//!
//! Off unless asked for, because the certificate is one this program made for itself:
//! a browser warns about it, and an operator who learns to click past that warning has
//! been taught something that costs more than plain text on a network they trust. A
//! paired phone is the reader it is for — it pins this certificate from pairing
//! material rather than asking anybody whether to trust it.
//!
//! The certificate is the core's, kept beside the configuration and made the first time
//! it is asked for, so every run after presents the same one and a phone paired once
//! goes on recognising this machine.

use std::sync::Arc;

use lemonfiber_api::guard::Binding;
use lemonfiber_core::companion::served::ServedReport;
use lemonfiber_core::PRODUCT;
use lemonfiber_error::codes::serve::UNSETTLED_PORT;
use lemonfiber_error::{Problem, Remedy};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::TlsAcceptor;

/// What a run serving encrypted presents, and what a phone would pin for it.
#[derive(Clone)]
pub(crate) struct Encrypting {
    /// What each connection is handed to before anything is read from it.
    pub(crate) acceptor: TlsAcceptor,
    /// SHA-256 over the certificate's DER encoding, lower-case hex.
    pub(crate) fingerprint: String,
}

/// Whether a run asked to encrypt named the port it serves on.
///
/// # Errors
///
/// The [`Problem`] to report where it was asked to encrypt and named no port.
pub(crate) fn port_settled(tls: bool, port: Option<u16>) -> Result<(), Box<Problem>> {
    if tls && port.is_none() {
        return Err(Box::new(unsettled()));
    }
    Ok(())
}

/// What this run presents, from the certificate settling how it is served kept or made,
/// or nothing where it is not encrypted.
///
/// # Errors
///
/// The [`Problem`] to report where the certificate kept would not serve.
pub(crate) fn encrypting(served: &ServedReport) -> Result<Option<Encrypting>, Box<Problem>> {
    let Some(kept) = served.presented.as_ref() else {
        return Ok(None);
    };
    let (certificate, key) = kept.presented();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .ok()
    .and_then(|builder| {
        builder
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(certificate)],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)),
            )
            .ok()
    })
    .ok_or_else(|| {
        Box::new(ServedReport::uncertified(
            "the certificate kept would not serve",
        ))
    })?;
    Ok(Some(Encrypting {
        acceptor: TlsAcceptor::from(Arc::new(config)),
        fingerprint: kept.fingerprint.clone(),
    }))
}

/// What a request has to name to be answered by a run on `port`, as settling how it is
/// served named it.
pub(crate) fn bound(served: &ServedReport, port: u16, network: bool) -> Binding {
    Binding {
        port,
        beyond: network,
        named: served.named.clone(),
    }
}

/// Encrypted was asked for with no port named.
fn unsettled() -> Problem {
    Problem::new(
        UNSETTLED_PORT,
        format!("{PRODUCT} serves encrypted only on a port you name"),
        "Serving encrypted is for a paired phone, and a phone keeps the address it was \
         given — a port chosen afresh every run would be one it stopped reaching the next \
         time this started.",
        Remedy::new("Name the port").with_detail(format!("{PRODUCT} ui --tls --port <port>")),
    )
}

#[cfg(test)]
mod tests;
