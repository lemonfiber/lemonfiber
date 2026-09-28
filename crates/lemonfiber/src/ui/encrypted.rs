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
use lemonfiber_core::app::Ctx;
use lemonfiber_core::companion::{answers_to, certificate};
use lemonfiber_core::error::codes::serve::{NO_CERTIFICATE, UNSETTLED_PORT};
use lemonfiber_core::error::{Problem, Remedy, Severity};
use lemonfiber_core::PRODUCT;
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

/// What this run presents, or nothing where it was not asked to encrypt.
///
/// # Errors
///
/// The [`Problem`] to report where it was asked to and cannot: no port that stays the
/// same, nowhere to keep a certificate, or a certificate that cannot be read or made.
pub(crate) fn encrypting(
    ctx: &Ctx,
    tls: bool,
    port: Option<u16>,
) -> Result<Option<Encrypting>, Box<Problem>> {
    if !tls {
        return Ok(None);
    }
    if port.is_none() {
        return Err(Box::new(unsettled()));
    }
    let directory = ctx.settings.companion.as_deref().ok_or_else(|| {
        Box::new(uncertified(
            "there is no configuration directory to keep one in",
        ))
    })?;
    let kept = certificate::kept_or_made(directory)
        .map_err(|why| Box::new(uncertified(&why.to_string())))?;
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
    .ok_or_else(|| Box::new(uncertified("the certificate kept would not serve")))?;
    Ok(Some(Encrypting {
        acceptor: TlsAcceptor::from(Arc::new(config)),
        fingerprint: kept.fingerprint,
    }))
}

/// What a request has to name to be answered by a run on `port`.
///
/// Served encrypted on a network, that includes the name pairing material gives a
/// phone, which is the only name a paired phone reaches it by. Anywhere else a name is
/// refused.
pub(crate) async fn bound(ctx: &Ctx, port: u16, network: bool, encrypted: bool) -> Binding {
    let named = if network && encrypted {
        answers_to(ctx, port).await
    } else {
        None
    };
    Binding {
        port,
        beyond: network,
        named,
    }
}

/// Encrypted was asked for with no port named.
fn unsettled() -> Problem {
    Problem::new(
        UNSETTLED_PORT,
        Severity::Error,
        format!("{PRODUCT} serves encrypted only on a port you name"),
        "Serving encrypted is for a paired phone, and a phone keeps the address it was \
         given — a port chosen afresh every run would be one it stopped reaching the next \
         time this started.",
        Remedy::new("Name the port").with_detail(format!("{PRODUCT} ui --tls --port <port>")),
    )
}

/// The certificate to present could not be had.
fn uncertified(why: &str) -> Problem {
    Problem::new(
        NO_CERTIFICATE,
        Severity::Error,
        format!("{PRODUCT} could not serve encrypted"),
        format!("It presents a certificate it keeps beside its configuration, and {why}."),
        Remedy::new("Replace the certificate, knowing every paired phone will need pairing again")
            .with_detail(format!("{PRODUCT} companion certificate --confirm")),
    )
}

#[cfg(test)]
mod tests;
