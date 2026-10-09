//! The certificates this machine presents where it serves encrypted: the web surface's,
//! and the guarded front door's in front of the media server.
//!
//! Made here and kept beside the configuration, because no public authority signs the
//! names a stack is reachable under — a phone pins each certificate from what the core
//! states instead, and a browser warns about it. Each is made once, the first time it is
//! wanted, and presented unchanged every run after, so a phone that pinned it once goes
//! on recognising this machine.
//!
//! **Nothing renews them.** Each is valid until long after anybody will be running this
//! build, so the only way one changes is somebody asking for it to be [`replaced`] — and
//! replacing it means every phone that pinned it refuses this machine until it pins the
//! new one, which is said before it is done rather than discovered afterwards.
//!
//! **What a phone pins is the certificate's digest**: SHA-256 over its DER encoding,
//! lower-case hex. Not the digest of its public key, which is the same length and a
//! different value, and would survive a replacement nobody was told about.

use std::path::Path;

use base64::Engine as _;

/// The certificate, as it is written down.
const CERTIFICATE: &str = "certificate.pem";

/// The private key it was made with, readable by its owner alone.
const KEY: &str = "key.pem";

/// The label a PEM block holding a certificate carries.
const CERTIFIED: &str = "CERTIFICATE";

/// The label a PEM block holding a PKCS #8 private key carries.
const PRIVATE_KEY: &str = "PRIVATE KEY";

/// The one name the certificate is made out to.
///
/// A phone does not check it — it pins the certificate itself — and a browser warns
/// whatever it says, because nothing it trusts signed it. So it names nothing that could
/// change: a certificate made out to this machine's address would describe the machine
/// wrongly the day the router hands it another, and replacing it then would un-pair
/// every phone for a reason nobody chose.
const NAMED: &str = "localhost";

/// The certificate this machine presents, and the key that goes with it.
#[derive(Clone, PartialEq, Eq)]
pub struct Kept {
    /// The certificate, DER-encoded.
    certificate: Vec<u8>,
    /// The private key, in PKCS #8.
    key: Vec<u8>,
    /// What a phone pins: SHA-256 over the certificate's DER encoding, lower-case hex.
    pub fingerprint: String,
}

impl std::fmt::Debug for Kept {
    /// Everything but the key, which is the one thing here that must not be printed.
    fn fmt(&self, into: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        into.debug_struct("Kept")
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

impl Kept {
    /// The certificate and its key as DER — the certificate, and the key in PKCS #8 —
    /// which is what a TLS server is handed.
    #[must_use]
    pub fn presented(&self) -> (Vec<u8>, Vec<u8>) {
        (self.certificate.clone(), self.key.clone())
    }

    /// A certificate and key as they were written down, or why they are not one.
    fn read(certificate: &str, key: &str) -> Result<Self, Unkept> {
        let certificate = der(certificate, CERTIFIED)
            .ok_or_else(|| Unkept::Unreadable(format!("{CERTIFICATE} is not a certificate")))?;
        let key = der(key, PRIVATE_KEY)
            .and_then(|bytes| rcgen::KeyPair::try_from(bytes).ok())
            .ok_or_else(|| Unkept::Unreadable(format!("{KEY} is not a private key")))?
            .serialize_der();
        Ok(Self {
            fingerprint: fingerprint(&certificate),
            certificate,
            key,
        })
    }
}

/// The bytes a PEM block labelled `label` holds, or nothing where there is no such block
/// or it is not base64.
fn der(pem: &str, label: &str) -> Option<Vec<u8>> {
    let (_, after) = pem.split_once(&format!("-----BEGIN {label}-----"))?;
    let (body, _) = after.split_once(&format!("-----END {label}-----"))?;
    let joined: String = body
        .chars()
        .filter(|letter| !letter.is_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(joined)
        .ok()
}

/// `der` as a PEM block labelled `label`: base64 in lines of sixty-four.
fn pem(label: &str, der: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    let mut lines = String::new();
    for (at, letter) in encoded.chars().enumerate() {
        if at > 0 && at % 64 == 0 {
            lines.push('\n');
        }
        lines.push(letter);
    }
    format!("-----BEGIN {label}-----\n{lines}\n-----END {label}-----\n")
}

/// Why there is no certificate to present.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unkept {
    /// What is written down is not a certificate and a key, or only half of them is.
    #[error("{0}")]
    Unreadable(String),
    /// A certificate could not be made or written down.
    #[error("{0}")]
    Unmade(String),
}

/// What a phone pins for a certificate: SHA-256 over its DER encoding, lower-case hex.
#[must_use]
pub fn fingerprint(der: &[u8]) -> String {
    crate::secret::render(ring::digest::digest(&ring::digest::SHA256, der).as_ref())
}

/// The certificate kept in `directory`, or nothing where none has been made.
///
/// # Errors
///
/// [`Unkept::Unreadable`] where only one of the two is there, or where either is not
/// what it should be. Neither is made again over it: a certificate replaced because it
/// could not be read is every paired phone refusing this machine for a reason nobody
/// was told.
pub fn kept(directory: &Path) -> Result<Option<Kept>, Unkept> {
    let certificate = std::fs::read_to_string(directory.join(CERTIFICATE)).ok();
    let key = std::fs::read_to_string(directory.join(KEY)).ok();
    match (certificate, key) {
        (None, None) => Ok(None),
        (Some(certificate), Some(key)) => Kept::read(&certificate, &key).map(Some),
        _ => Err(Unkept::Unreadable(format!(
            "only one of {CERTIFICATE} and {KEY} is there, and a certificate is presented with \
             the key it was made with"
        ))),
    }
}

/// The certificate kept in `directory`, made first where none has been.
///
/// # Errors
///
/// [`Unkept`] where what is kept cannot be read, or a new one cannot be made.
pub fn kept_or_made(directory: &Path) -> Result<Kept, Unkept> {
    match kept(directory)? {
        Some(held) => Ok(held),
        None => made(directory),
    }
}

/// A new certificate, written over whatever was kept in `directory`.
///
/// Every phone paired against the one it replaces refuses this machine until it is
/// paired again. That is said to the operator before this is reached.
///
/// # Errors
///
/// [`Unkept::Unmade`] where a certificate could not be made or written down.
pub fn replaced(directory: &Path) -> Result<Kept, Unkept> {
    made(directory)
}

/// Make a certificate and write it and its key down.
///
/// The key first, so a stop between the two leaves a key with no certificate — which is
/// refused as half of a pair when it is next read — rather than a certificate somebody
/// could pin whose key is gone.
fn made(directory: &Path) -> Result<Kept, Unkept> {
    let generated = rcgen::generate_simple_self_signed(vec![NAMED.to_owned()]);
    let made = generated.as_ref().map_err(unmade)?;
    let (certificate, key) = (
        pem(CERTIFIED, made.cert.der()),
        pem(PRIVATE_KEY, &made.signing_key.serialize_der()),
    );
    for (name, text) in [(KEY, &key), (CERTIFICATE, &certificate)] {
        crate::config::store::write(&directory.join(name), text)
            .map_err(|why| Unkept::Unmade(format!("{name} could not be written: {why}")))?;
    }
    Kept::read(&certificate, &key)
}

/// Why a certificate could not be made, as it is reported.
fn unmade(why: &rcgen::Error) -> Unkept {
    Unkept::Unmade(format!("a certificate could not be made: {why}"))
}

#[cfg(test)]
mod tests;
