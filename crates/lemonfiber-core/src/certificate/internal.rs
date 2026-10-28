//! The certificate the media server presents at its internal TLS port, where every service
//! the core hands a media server credential reaches it.
//!
//! Made for the media server's service name, signed by nothing outside the house, and kept
//! as a PKCS#12 with an empty password beneath that service's own configuration directory,
//! readable by the operator alone: the media server is handed its path and serves from it.
//! Each holder of a credential pins it by the same digest a phone pins the door by.

use std::path::{Path, PathBuf};

use p12_keystore::{Certificate, KeyStore, KeyStoreEntry, PrivateKey, PrivateKeyChain};

use super::{fingerprint, Unkept};

/// Where the certificate is kept beneath the media server's configuration directory.
pub const INTERNAL: &str = "lemonfiber/internal.p12";

/// The password the PKCS#12 is written with, which the media server is not handed.
const PASSWORD: &str = "";

/// The name the certificate and its key are filed under inside the PKCS#12.
const ALIAS: &str = "lemonfiber-internal";

/// The certificate as it is written, and the digest a holder pins it by.
#[derive(Clone, PartialEq, Eq)]
pub struct Internal {
    /// The PKCS#12 holding the certificate and its key.
    pkcs12: Vec<u8>,
    /// SHA-256 over the certificate's DER encoding, lower-case hex.
    pub fingerprint: String,
}

impl std::fmt::Debug for Internal {
    /// The fingerprint alone: the PKCS#12 holds the key.
    fn fmt(&self, into: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        into.debug_struct("Internal")
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

impl Internal {
    /// The PKCS#12 as it is written.
    #[must_use]
    pub fn pkcs12(&self) -> &[u8] {
        &self.pkcs12
    }
}

/// Where the certificate is kept beneath the configuration directory `config`.
#[must_use]
pub fn path(config: &Path) -> PathBuf {
    config.join(INTERNAL)
}

/// A new certificate made out to `service`, the name the media server is reached by inside
/// the stack.
///
/// # Errors
///
/// [`Unkept::Unmade`] where the certificate or its PKCS#12 could not be made.
pub fn made_for(service: &str) -> Result<Internal, Unkept> {
    sealed(service).ok_or_else(|| {
        Unkept::Unmade(format!(
            "a certificate made out to {service:?} could not be made or written as {INTERNAL}"
        ))
    })
}

/// The certificate made out to `service` and its PKCS#12, or nothing where either could
/// not be made.
fn sealed(service: &str) -> Option<Internal> {
    let mut params = rcgen::CertificateParams::new(vec![service.to_owned()]).ok()?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, service);
    let signing = rcgen::KeyPair::generate().ok()?;
    let certificate = params.self_signed(&signing).ok()?.der().to_vec();
    let chain = PrivateKeyChain::new(
        ring::digest::digest(&ring::digest::SHA256, &certificate)
            .as_ref()
            .to_vec(),
        PrivateKey::from_der(&signing.serialize_der()).ok()?,
        [Certificate::from_der(&certificate).ok()?],
    );
    let mut store = KeyStore::new();
    store.add_entry(ALIAS, KeyStoreEntry::PrivateKeyChain(chain));
    Some(Internal {
        fingerprint: fingerprint(&certificate),
        pkcs12: store.writer(PASSWORD).write().ok()?,
    })
}

/// The certificate the PKCS#12 `bytes` holds, read with the empty password.
///
/// # Errors
///
/// [`Unkept::Unreadable`] where it is not a PKCS#12 holding a certificate and its key.
pub fn read(bytes: &[u8]) -> Result<Internal, Unkept> {
    let store = KeyStore::from_pkcs12(bytes, PASSWORD, p12_keystore::Pkcs12ImportPolicy::Strict)
        .map_err(|why| Unkept::Unreadable(format!("{INTERNAL} does not read: {why}")))?;
    let certificate = store
        .private_key_chain()
        .and_then(|(_, chain)| chain.certs().first())
        .ok_or_else(|| {
            Unkept::Unreadable(format!("{INTERNAL} holds no certificate with its key"))
        })?;
    Ok(Internal {
        fingerprint: fingerprint(certificate.as_der()),
        pkcs12: bytes.to_vec(),
    })
}

/// The certificate kept beneath `config`, or nothing where none has been made.
///
/// # Errors
///
/// [`Unkept::Unreadable`] where what is kept is not one.
pub fn kept(config: &Path) -> Result<Option<Internal>, Unkept> {
    match std::fs::read(path(config)) {
        Ok(bytes) => read(&bytes).map(Some),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(why) => Err(Unkept::Unreadable(format!(
            "{INTERNAL} could not be read: {why}"
        ))),
    }
}

/// `made` written beneath `config`, readable by its owner alone, in place of whatever was
/// kept there.
///
/// # Errors
///
/// [`Unkept::Unmade`] where it could not be written.
pub fn written(config: &Path, made: &Internal) -> Result<(), Unkept> {
    crate::config::store::write_bytes(&path(config), &made.pkcs12)
        .map_err(|why| Unkept::Unmade(format!("{INTERNAL} could not be written: {why}")))
}

/// The certificate kept beneath `config`, made out to `service` and written first where
/// none has been.
///
/// # Errors
///
/// [`Unkept`] where what is kept cannot be read, or a new one cannot be made or written.
pub fn kept_or_made(config: &Path, service: &str) -> Result<Internal, Unkept> {
    if let Some(held) = kept(config)? {
        return Ok(held);
    }
    let made = made_for(service)?;
    written(config, &made)?;
    Ok(made)
}

#[cfg(test)]
mod tests;
