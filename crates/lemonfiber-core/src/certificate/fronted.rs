//! The certificate authority the request gate answers for plex.tv under, in `fronted` mode,
//! and the one certificate it signs.
//!
//! The authority is made for the gate alone. It signs one certificate naming only plex.tv
//! and discover.provider.plex.tv, it may vouch for nothing outside plex.tv, and its key is
//! never written down, so nothing is signed by it again. The gate is handed the certificate
//! and its key, readable by the operator alone; the request service is handed the
//! authority's certificate to trust.

use std::path::Path;

use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, GeneralSubtree, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, NameConstraints,
};

use super::{der, pem, Unkept, CERTIFIED, PRIVATE_KEY};
use crate::ports::filesystem::{Confined, Readable};

/// The hosts the certificate names, which the request service resolves to the gate.
pub const HOSTS: [&str; 2] = ["plex.tv", "discover.provider.plex.tv"];

/// The domain the authority may vouch for names beneath.
const DOMAIN: &str = "plex.tv";

/// What the authority is called in its certificate.
const AUTHORITY_NAME: &str = "lemonfiber request gate, for plex.tv alone";

/// Where the certificate is written beneath the gate's configuration directory.
pub const CERTIFICATE: &str = "plex-tv/certificate.pem";

/// Where its key is written beneath the gate's configuration directory.
pub const KEY: &str = "plex-tv/key.pem";

/// Where the authority's certificate is written beneath the request service's
/// configuration directory.
pub const AUTHORITY: &str = "plex-tv/authority.pem";

/// An authority's certificate and the one certificate it signed with its key, as PEM.
#[derive(Clone, PartialEq, Eq)]
pub struct Fronted {
    /// The authority's certificate.
    authority: String,
    /// The certificate it signed.
    certificate: String,
    /// The certificate's private key.
    key: String,
}

impl std::fmt::Debug for Fronted {
    /// Nothing but that it is one: the key is the one thing here that must not be printed.
    fn fmt(&self, into: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        into.debug_struct("Fronted").finish_non_exhaustive()
    }
}

/// The configuration directories the files are written beneath: the gate's and the
/// request service's.
#[derive(Debug, Clone, Copy)]
pub struct Beneath<'a> {
    /// The gate's configuration directory.
    pub gate: &'a Path,
    /// The request service's configuration directory.
    pub requests: &'a Path,
}

/// A new authority and the certificate it signs for [`HOSTS`].
///
/// # Errors
///
/// [`Unkept::Unmade`] where either cannot be made.
pub fn made() -> Result<Fronted, Unkept> {
    let mut authority = CertificateParams::new(Vec::<String>::new())?;
    authority.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    authority
        .distinguished_name
        .push(DnType::CommonName, AUTHORITY_NAME);
    authority.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    authority.name_constraints = Some(NameConstraints {
        permitted_subtrees: vec![GeneralSubtree::DnsName(DOMAIN.to_owned())],
        excluded_subtrees: Vec::new(),
    });
    let signing = KeyPair::generate()?;
    let signed_authority = authority.self_signed(&signing)?;
    let issuer = Issuer::new(authority, signing);
    let mut leaf = CertificateParams::new(HOSTS.map(str::to_owned).to_vec())?;
    leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let key = KeyPair::generate()?;
    let certificate = leaf.signed_by(&key, &issuer)?;
    Ok(Fronted {
        authority: pem(CERTIFIED, signed_authority.der()),
        certificate: pem(CERTIFIED, certificate.der()),
        key: pem(PRIVATE_KEY, &key.serialize_der()),
    })
}

/// Whether what is written beneath `at` is one authority and the certificate it signed
/// for [`HOSTS`] alone, with that certificate's key beside it.
///
/// # Errors
///
/// [`Unkept::Unreadable`] where a file is reached through a link or cannot be read.
pub fn kept(confined: &dyn Confined, at: Beneath<'_>) -> Result<bool, Unkept> {
    let read = |within: &Path, name: &str| {
        confined
            .bytes(&within.join(name), within)
            .map_err(|fault| {
                Unkept::Unreadable(format!("{name} could not be read: {}", fault.message))
            })
            .map(|bytes| bytes.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
    };
    let (Some(authority), Some(certificate), Some(key)) = (
        read(at.requests, AUTHORITY)?,
        read(at.gate, CERTIFICATE)?,
        read(at.gate, KEY)?,
    ) else {
        return Ok(false);
    };
    Ok(signed_for_hosts(&authority, &certificate)
        && der(&key, PRIVATE_KEY).is_some_and(|key| KeyPair::try_from(key).is_ok()))
}

/// Whether `certificate` names [`HOSTS`] alone and was signed with `authority`'s key.
fn signed_for_hosts(authority: &str, certificate: &str) -> bool {
    let (Some(authority), Some(certificate)) =
        (der(authority, CERTIFIED), der(certificate, CERTIFIED))
    else {
        return false;
    };
    let (Ok((_, authority)), Ok((_, certificate))) = (
        x509_parser::parse_x509_certificate(&authority),
        x509_parser::parse_x509_certificate(&certificate),
    ) else {
        return false;
    };
    let mut named: Vec<String> = certificate
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|names| {
            names
                .value
                .general_names
                .iter()
                .filter_map(|name| match name {
                    x509_parser::extensions::GeneralName::DNSName(dns) => Some((*dns).to_owned()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    named.sort();
    let mut hosts = HOSTS.map(str::to_owned).to_vec();
    hosts.sort();
    let signed = ring::signature::UnparsedPublicKey::new(
        &ring::signature::ECDSA_P256_SHA256_ASN1,
        &authority.public_key().subject_public_key.data,
    )
    .verify(
        certificate.tbs_certificate.as_ref(),
        &certificate.signature_value.data,
    )
    .is_ok();
    named == hosts && signed
}

/// `made` written beneath `at`: the certificate and its key beneath the gate's directory,
/// readable by the operator alone, the key first, then the authority's certificate
/// beneath the request service's.
///
/// # Errors
///
/// [`Unkept::Unmade`] where a file could not be written.
pub fn written(confined: &dyn Confined, at: Beneath<'_>, made: &Fronted) -> Result<(), Unkept> {
    for (within, name, text, readable) in [
        (at.gate, KEY, &made.key, Readable::Private),
        (at.gate, CERTIFICATE, &made.certificate, Readable::Private),
        (at.requests, AUTHORITY, &made.authority, Readable::Shared),
    ] {
        confined
            .replaced(&within.join(name), within, text.as_bytes(), readable)
            .map_err(|fault| {
                Unkept::Unmade(format!("{name} could not be written: {}", fault.message))
            })?;
    }
    Ok(())
}

/// What is written beneath `at` kept where it holds, and made and written anew where it
/// does not: whether it was written.
///
/// # Errors
///
/// [`Unkept`] where it cannot be read, made or written.
pub fn kept_or_made(confined: &dyn Confined, at: Beneath<'_>) -> Result<bool, Unkept> {
    if kept(confined, at)? {
        return Ok(false);
    }
    written(confined, at, &made()?)?;
    Ok(true)
}

/// Every file beneath `at` taken away: the authority the request service trusted, and
/// the certificate and key the gate answered under.
///
/// # Errors
///
/// [`Unkept::Unmade`] where one could not be taken away.
pub fn withdrawn(confined: &dyn Confined, at: Beneath<'_>) -> Result<(), Unkept> {
    for (within, name) in [
        (at.requests, AUTHORITY),
        (at.gate, CERTIFICATE),
        (at.gate, KEY),
    ] {
        confined
            .removed(&within.join(name), within)
            .map_err(|fault| {
                Unkept::Unmade(format!("{name} could not be taken away: {}", fault.message))
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
