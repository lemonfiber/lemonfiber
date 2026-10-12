//! Apple App Attest: an attestation object, in CBOR, vouching for a genuine instance of
//! an app and for a hash it signed.
//!
//! App Attest vouches for the app, not for any key the app made. The phone attests with
//! the SHA-256 of the device key's public half as the client data hash, so what is read
//! here is that the genuine app, on a genuine device, asked for exactly that public half
//! to be registered, and the key's backing is recorded as the enclave the app placed it
//! in, not as anything the hardware proved.

use std::time::SystemTime;

use x509_parser::asn1_rs::Class;

use super::cbor::Cbor;
use super::chain::{self, Dated};
use super::der::{self, extension, read_one, universal, within};
use super::{Attested, Unattested};
use crate::device_keys::identity::AppIdentity;
use crate::device_keys::{Backing, PublicKey};
use crate::secret::render;

/// The root an App Attest attestation is read back to.
const ROOTS: [&[u8]; 1] = [include_bytes!("roots/apple-app-attestation.der")];

/// The format an App Attest attestation object names.
const FORMAT: &str = "apple-appattest";

/// The extension of the credential certificate the nonce is written in.
const NONCE: &str = "1.2.840.113635.100.8.2";

/// The tag the nonce is written under inside that extension.
const NONCE_TAG: u32 = 1;

/// What the authenticator data names the environment that attested: production, and
/// development.
const ENVIRONMENTS: [&[u8; 16]; 2] = [b"appattest\0\0\0\0\0\0\0", b"appattestdevelop"];

/// Bytes in a SHA-256.
const DIGEST: usize = 32;

/// Where each field of the authenticator data begins.
mod at {
    /// The SHA-256 of the App ID.
    pub(super) const APP: usize = 0;
    /// The counter, which is zero in an attestation.
    pub(super) const COUNTER: usize = 33;
    /// The environment.
    pub(super) const ENVIRONMENT: usize = 37;
    /// The length of the credential id.
    pub(super) const CREDENTIAL_LENGTH: usize = 53;
    /// The credential id.
    pub(super) const CREDENTIAL: usize = 55;
}

/// What an attestation object holds that verifying it reads.
struct Object {
    /// The credential certificate, then the certificates that issued it.
    chain: Vec<Vec<u8>>,
    /// The authenticator data.
    authenticator: Vec<u8>,
}

/// What `object`, an App Attest attestation object, vouches for `key` at `now`, from an
/// app among `trusted`.
///
/// # Errors
///
/// [`Unattested`] naming why nothing is vouched for.
pub(super) fn attested(
    object: &[u8],
    key: &PublicKey,
    trusted: &[AppIdentity],
    now: SystemTime,
) -> Result<Attested, Unattested> {
    attested_under(&ROOTS, object, key, trusted, now)
}

/// What `object` vouches for `key` when read back to `roots`.
fn attested_under(
    roots: &[&[u8]],
    object: &[u8],
    key: &PublicKey,
    trusted: &[AppIdentity],
    now: SystemTime,
) -> Result<Attested, Unattested> {
    let client = ring::digest::digest(&ring::digest::SHA256, key.encoded());
    let read = read(object).ok_or(Unattested::Unverified)?;
    let app = verified(roots, &read, client.as_ref(), now).ok_or(Unattested::Unverified)?;
    let app = trusted
        .iter()
        .find(|identity| {
            identity.app_id().is_some_and(|id| {
                ring::digest::digest(&ring::digest::SHA256, id.as_bytes()).as_ref() == app
            })
        })
        .cloned()
        .ok_or_else(|| {
            Unattested::Untrusted(format!(
                "the iPhone app whose identity hashes to {}",
                render(app)
            ))
        })?;
    Ok(Attested {
        backing: Backing::Enclave,
        app,
    })
}

/// The SHA-256 of the App ID `object` vouches for, where it is an attestation of
/// `client`, the client data hash, read back to one of `roots` at `now`.
fn verified<'a>(
    roots: &[&[u8]],
    object: &'a Object,
    client: &[u8],
    now: SystemTime,
) -> Option<&'a [u8]> {
    let certificates = chain::read(&object.chain)?;
    let credential = certificates.first()?;
    if !chain::verified(&certificates, roots, now, Dated::Every) {
        return None;
    }
    let data = object.authenticator.as_slice();
    let nonce = ring::digest::digest(&ring::digest::SHA256, &[data, client].concat());
    if nonced(extension(credential, NONCE)?)? != nonce.as_ref() {
        return None;
    }
    let key_id = ring::digest::digest(
        &ring::digest::SHA256,
        credential.public_key().subject_public_key.data.as_ref(),
    );
    let counter = data.get(at::COUNTER..at::ENVIRONMENT)?;
    let environment = data.get(at::ENVIRONMENT..at::CREDENTIAL_LENGTH)?;
    let length = data.get(at::CREDENTIAL_LENGTH..at::CREDENTIAL)?;
    let length = usize::from(u16::from_be_bytes([*length.first()?, *length.get(1)?]));
    let credential_id = data.get(at::CREDENTIAL..at::CREDENTIAL.checked_add(length)?)?;
    let attested = counter.iter().all(|byte| *byte == 0)
        && ENVIRONMENTS
            .iter()
            .any(|named| named.as_slice() == environment)
        && credential_id == key_id.as_ref();
    attested
        .then(|| data.get(at::APP..at::APP + DIGEST))
        .flatten()
}

/// The nonce the credential certificate's extension holds.
fn nonced(value: &[u8]) -> Option<&[u8]> {
    let fields = within(value, der::tag::SEQUENCE)?;
    let tagged = fields
        .iter()
        .find(|field| field.class() == Class::ContextSpecific && field.tag().0 == NONCE_TAG)?;
    let octets = read_one(tagged.data)?;
    Some(universal(&octets, der::tag::OCTETS)?.data)
}

/// What the attestation object `bytes` holds, where it is one in the App Attest format:
/// a map of `fmt`, `attStmt` and `authData`, each once and nothing else, and nothing
/// after it.
fn read(bytes: &[u8]) -> Option<Object> {
    let mut cbor = Cbor::over(bytes);
    let mut format = None;
    let mut chain = None;
    let mut authenticator = None;
    for _ in 0..cbor.map()? {
        match cbor.text()?.as_str() {
            "fmt" => once(&mut format, cbor.text()?)?,
            "attStmt" => once(&mut chain, statement(&mut cbor)?)?,
            "authData" => once(&mut authenticator, cbor.bytes()?)?,
            _ => return None,
        }
    }
    (cbor.finished() && format.as_deref() == Some(FORMAT)).then_some(())?;
    Some(Object {
        chain: chain?,
        authenticator: authenticator?,
    })
}

/// The certificates an attestation statement carries: a map of `x5c` and, where Apple
/// gives one, `receipt`, each once and nothing else.
fn statement(cbor: &mut Cbor<'_>) -> Option<Vec<Vec<u8>>> {
    let mut chain = None;
    let mut receipt = None;
    for _ in 0..cbor.map()? {
        match cbor.text()?.as_str() {
            "x5c" => {
                let mut certificates = Vec::new();
                for _ in 0..cbor.array()? {
                    certificates.push(cbor.bytes()?);
                }
                once(&mut chain, certificates)?;
            }
            "receipt" => once(&mut receipt, cbor.bytes()?)?,
            _ => return None,
        }
    }
    chain
}

/// Fill `slot` with `value`, or nothing where a field came twice.
fn once<T>(slot: &mut Option<T>, value: T) -> Option<()> {
    slot.is_none().then(|| *slot = Some(value))
}

#[cfg(test)]
mod tests;
