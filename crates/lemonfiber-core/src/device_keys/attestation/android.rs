//! Android key attestation: a chain whose first certificate is the key's own, carrying
//! the key description the phone's secure hardware wrote.
//!
//! The key's certificate holds the key itself, so the public half it covers is read off
//! it rather than out of a challenge. The description says where the key lives, that the
//! hardware made it rather than took it in from outside, and which app made it, by its
//! package and the digests of its signing certificates.

use std::time::SystemTime;

use x509_parser::asn1_rs::{Any, Class};
use x509_parser::prelude::X509Certificate;

use super::chain::{self, Dated};
use super::der::{self, children, extension, read_one, universal, within};
use super::status::{self, Snapshot};
use super::{Attested, Unattested};
use crate::device_keys::identity::AppIdentity;
use crate::device_keys::{Backing, PublicKey};
use crate::secret::render;

/// The roots an Android key attestation is read back to: Google's RSA root, in every
/// issue of its one key, and its elliptic-curve root.
const ROOTS: [&[u8]; 2] = [
    include_bytes!("roots/google-rsa.der"),
    include_bytes!("roots/google-ec.der"),
];

/// The extension the key description is written in.
const KEY_DESCRIPTION: &str = "1.3.6.1.4.1.11129.2.1.17";

/// The authorization naming the app that made the key.
const APPLICATION: u32 = 709;

/// The authorization saying where the key came from.
const ORIGIN: u32 = 702;

/// What an app is called whose identity the attestation spells in a form no setting could.
const UNSPELT: &str = "an Android app whose package name is not one Android allows";

/// The origin of a key the hardware made itself.
const GENERATED: u8 = 0;

/// Where the description says something lives, in the order the format numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    /// The operating system's own keystore.
    Software,
    /// The processor's trusted environment.
    TrustedEnvironment,
    /// A separate secure chip.
    StrongBox,
}

impl Level {
    /// The level the format numbers `number`.
    const fn numbered(number: u32) -> Option<Self> {
        match number {
            0 => Some(Self::Software),
            1 => Some(Self::TrustedEnvironment),
            2 => Some(Self::StrongBox),
            _ => None,
        }
    }
}

/// What a key description says, as far as registering a device key reads it.
struct Description {
    /// Where the attestation was made.
    attested: Level,
    /// Where the key lives.
    held: Level,
    /// Whether the hardware says it made the key, rather than took it in from outside.
    generated: bool,
    /// The packages of the app that made the key.
    packages: Vec<String>,
    /// The SHA-256 of each of that app's signing certificates, in lower-case hex.
    signers: Vec<String>,
}

/// What `chain` vouches for `key`, the key's own certificate first, at `now`, from an app
/// among `trusted`.
///
/// # Errors
///
/// [`Unattested`] naming why nothing is vouched for.
pub(super) fn attested(
    chain: &[Vec<u8>],
    key: &PublicKey,
    trusted: &[AppIdentity],
    now: SystemTime,
) -> Result<Attested, Unattested> {
    attested_under(&ROOTS, status::carried(), chain, key, trusted, now)
}

/// What `chain` vouches for `key` when read back to `roots`, none of its certificates
/// named in `listed`; nothing is vouched for where there is no list to read.
fn attested_under(
    roots: &[&[u8]],
    listed: Option<&Snapshot>,
    chain: &[Vec<u8>],
    key: &PublicKey,
    trusted: &[AppIdentity],
    now: SystemTime,
) -> Result<Attested, Unattested> {
    let certificates = chain::read(chain).ok_or(Unattested::Unverified)?;
    let own = certificates.first().ok_or(Unattested::Unverified)?;
    let described = described(own).ok_or(Unattested::Unverified)?;
    let backing = match described.attested.min(described.held) {
        Level::Software => return Err(Unattested::InSoftware),
        Level::TrustedEnvironment => Backing::Tee,
        Level::StrongBox => Backing::Strongbox,
    };
    if !described.generated {
        return Err(Unattested::InSoftware);
    }
    if own.public_key().raw != key.encoded()
        || !chain::verified(&certificates, roots, now, Dated::ButTheKeys)
    {
        return Err(Unattested::Unverified);
    }
    match listed {
        None => return Err(Unattested::Unverified),
        Some(listed) if listed.names_any(&certificates) => return Err(Unattested::Revoked),
        Some(_) => {}
    }
    let app = trusted
        .iter()
        .find(|identity| made_by(&described, identity))
        .cloned()
        .ok_or_else(|| Unattested::Untrusted(named(&described)))?;
    Ok(Attested { backing, app })
}

/// Whether `identity` is the app the description names.
fn made_by(described: &Description, identity: &AppIdentity) -> bool {
    match identity {
        AppIdentity::Android {
            package,
            certificate,
        } => {
            described.packages.iter().any(|named| named == package)
                && described.signers.iter().any(|signer| signer == certificate)
        }
        AppIdentity::Ios { .. } => false,
    }
}

/// The app the description names, as the setting would spell it, or as an app whose
/// identity cannot be spelt where its package is not one Android allows: the phone wrote
/// it, and it reaches the operator's screen.
fn named(described: &Description) -> String {
    let package = described.packages.first().map_or("", String::as_str);
    let signer = described.signers.first().map_or("", String::as_str);
    let written = AppIdentity::Android {
        package: package.to_owned().into(),
        certificate: signer.to_owned().into(),
    }
    .written();
    AppIdentity::read(&written).map_or_else(|| UNSPELT.to_owned(), |_| written)
}

/// The key description `own` carries, or nothing where it carries none that reads.
fn described(own: &X509Certificate<'_>) -> Option<Description> {
    let fields = within(extension(own, KEY_DESCRIPTION)?, der::tag::SEQUENCE)?;
    let attested = level(fields.get(1)?)?;
    let held = level(fields.get(3)?)?;
    let hardware = universal(fields.get(7)?, der::tag::SEQUENCE)?;
    let generated = authorization(hardware, ORIGIN)
        .and_then(read_one)
        .and_then(|origin| {
            universal(&origin, der::tag::INTEGER).map(|origin| origin.data == [GENERATED])
        })
        .unwrap_or(false);
    let application = [fields.get(6)?, fields.get(7)?]
        .into_iter()
        .filter_map(|list| universal(list, der::tag::SEQUENCE))
        .find_map(|list| authorization(list, APPLICATION))?;
    let named = universal(&read_one(application)?, der::tag::OCTETS)?.data;
    let parts = within(named, der::tag::SEQUENCE)?;
    let packages = members(parts.first()?)?
        .iter()
        .filter_map(|package| package_name(package))
        .collect();
    let signers = members(parts.get(1)?)?
        .iter()
        .filter_map(|digest| universal(digest, der::tag::OCTETS).map(|octets| render(octets.data)))
        .collect();
    Some(Description {
        attested,
        held,
        generated,
        packages,
        signers,
    })
}

/// The package one entry of the app's packages names.
fn package_name(entry: &Any<'_>) -> Option<String> {
    let fields = children(universal(entry, der::tag::SEQUENCE)?.data)?;
    let name = universal(fields.first()?, der::tag::OCTETS)?;
    String::from_utf8(name.data.to_vec()).ok()
}

/// The value the authorization tagged `number` holds in `list`, where it holds one.
fn authorization<'a>(list: &Any<'a>, number: u32) -> Option<&'a [u8]> {
    children(list.data)?
        .into_iter()
        .find(|entry| entry.class() == Class::ContextSpecific && entry.tag().0 == number)
        .map(|entry| entry.data)
}

/// The level one enumerated field says.
fn level(field: &Any<'_>) -> Option<Level> {
    let value = universal(field, der::tag::ENUMERATED)?;
    let [number] = <[u8; 1]>::try_from(value.data).ok()?;
    Level::numbered(u32::from(number))
}

/// The members of a set.
fn members<'a>(set: &Any<'a>) -> Option<Vec<Any<'a>>> {
    children(universal(set, der::tag::SET)?.data)
}

#[cfg(test)]
mod tests;
