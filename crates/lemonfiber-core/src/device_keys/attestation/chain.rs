//! A certificate chain read back to a root this build ships.
//!
//! Each certificate is signed by the next, each signer is a certificate authority, and
//! the last is signed by a shipped root's key under that root's name. A chain ending in
//! an older issue of the same root, under the same name and key, is read the same as one
//! ending in the newest. Nothing here asks anybody anything: no
//! revocation list is fetched, and the roots are the ones this build carries.

use std::time::{SystemTime, UNIX_EPOCH};

use ring::signature::{
    VerificationAlgorithm, ECDSA_P256_SHA256_ASN1, ECDSA_P256_SHA384_ASN1, ECDSA_P384_SHA256_ASN1,
    ECDSA_P384_SHA384_ASN1, RSA_PKCS1_2048_8192_SHA256, RSA_PKCS1_2048_8192_SHA384,
    RSA_PKCS1_2048_8192_SHA512,
};
use x509_parser::oid_registry::Oid;
use x509_parser::oid_registry::{
    OID_EC_P256, OID_NIST_EC_P384, OID_PKCS1_SHA256WITHRSA, OID_PKCS1_SHA384WITHRSA,
    OID_PKCS1_SHA512WITHRSA, OID_SIG_ECDSA_WITH_SHA256, OID_SIG_ECDSA_WITH_SHA384,
};
use x509_parser::prelude::{ASN1Time, FromDer as _, ParsedExtension, X509Certificate};
use x509_parser::x509::SubjectPublicKeyInfo;

/// The most certificates a chain may hold, its root included.
const LONGEST: usize = 10;

/// Which certificates' dates are held to the moment of checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dated {
    /// Every certificate's.
    Every,
    /// Every one but the key's own, whose dates the phone wrote and no authority vouched
    /// for.
    ButTheKeys,
}

/// The certificates `chain` holds, read, the key's own first.
///
/// Nothing where any of them does not read as one certificate with nothing after it or
/// carries a critical extension this cannot read, or the chain is empty or longer than
/// any a phone makes.
pub(crate) fn read(chain: &[Vec<u8>]) -> Option<Vec<X509Certificate<'_>>> {
    if chain.is_empty() || chain.len() > LONGEST {
        return None;
    }
    chain
        .iter()
        .map(|der| match X509Certificate::from_der(der) {
            Ok((rest, read)) if rest.is_empty() && understood(&read) => Some(read),
            _ => None,
        })
        .collect()
}

/// Whether every critical extension `certificate` carries is one this reads and holds a
/// chain to, and the algorithm it is signed with is the one its signed part names.
///
/// Name, policy and any-policy constraints read but are not held to, so a certificate
/// marking one critical is refused rather than read as though it were met.
fn understood(certificate: &X509Certificate<'_>) -> bool {
    let extensions_held = certificate.extensions().iter().all(|extension| {
        let parsed = extension.parsed_extension();
        !extension.critical
            || !(parsed.unsupported()
                || parsed.error().is_some()
                || matches!(
                    parsed,
                    ParsedExtension::NameConstraints(_)
                        | ParsedExtension::PolicyConstraints(_)
                        | ParsedExtension::InhibitAnyPolicy(_)
                ))
    });
    let outer = &certificate.signature_algorithm;
    let inner = &certificate.tbs_certificate.signature;
    let algorithm_held = outer.algorithm == inner.algorithm
        && outer
            .parameters
            .as_ref()
            .map(x509_parser::asn1_rs::Any::as_bytes)
            == inner
                .parameters
                .as_ref()
                .map(x509_parser::asn1_rs::Any::as_bytes);
    extensions_held && algorithm_held
}

/// Whether `certificates`, the key's own first, read back to one of `roots` at `now`.
///
/// The key's own certificate is never an anchor: at least one authority above it has to
/// have signed it. Each authority is one by its basic constraints, may sign certificates
/// where it says what it may do, and holds to the path length it allows; and the last
/// certificate is signed by a shipped root's key under that root's name, whether it is a
/// certificate the root issued or an issue of the root itself.
pub(crate) fn verified(
    certificates: &[X509Certificate<'_>],
    roots: &[&[u8]],
    now: SystemTime,
    dated: Dated,
) -> bool {
    let Some(at) = moment(now) else {
        return false;
    };
    let Some(last) = certificates.last() else {
        return false;
    };
    let dates_hold = certificates
        .iter()
        .enumerate()
        .filter(|(index, _)| dated == Dated::Every || *index > 0)
        .all(|(_, certificate)| certificate.validity().is_valid_at(at));
    let links_hold = certificates
        .windows(2)
        .all(|pair| matches!(pair, [below, above] if issued(below, above)));
    let authorities_hold = certificates
        .iter()
        .enumerate()
        .skip(1)
        .all(|(index, signer)| authority(signer, index - 1));
    certificates.len() > 1 && dates_hold && links_hold && authorities_hold && anchored(last, roots)
}

/// Whether `above` issued and signed `below`.
fn issued(below: &X509Certificate<'_>, above: &X509Certificate<'_>) -> bool {
    below.issuer().as_raw() == above.subject().as_raw() && signed_by(below, above.public_key())
}

/// Whether `signer` is an authority that may sign certificates, with `below` authorities
/// between it and the key's own certificate.
fn authority(signer: &X509Certificate<'_>, below: usize) -> bool {
    let constrained = signer
        .basic_constraints()
        .ok()
        .flatten()
        .is_some_and(|held| {
            held.value.ca
                && held.value.path_len_constraint.is_none_or(|longest| {
                    usize::try_from(longest).is_ok_and(|longest| below <= longest)
                })
        });
    let may_sign = signer
        .key_usage()
        .is_ok_and(|usage| usage.is_none_or(|usage| usage.value.key_cert_sign()));
    constrained && may_sign
}

/// Whether one of `roots` signed `last` under its own name.
fn anchored(last: &X509Certificate<'_>, roots: &[&[u8]]) -> bool {
    roots
        .iter()
        .filter_map(|root| X509Certificate::from_der(root).ok().map(|(_, root)| root))
        .any(|root| {
            last.issuer().as_raw() == root.subject().as_raw() && signed_by(last, root.public_key())
        })
}

/// Whether `certificate`'s signature is `key`'s, under an algorithm this accepts.
///
/// RSA with SHA-256 or wider, and ECDSA on P-256 or P-384 with SHA-256 or SHA-384. SHA-1
/// and anything else is refused, whatever a certificate names.
fn signed_by(certificate: &X509Certificate<'_>, key: &SubjectPublicKeyInfo<'_>) -> bool {
    let Some(algorithm) = algorithm(certificate, key) else {
        return false;
    };
    ring::signature::UnparsedPublicKey::new(algorithm, key.subject_public_key.data.as_ref())
        .verify(
            certificate.tbs_certificate.as_ref(),
            certificate.signature_value.data.as_ref(),
        )
        .is_ok()
}

/// The verification `certificate`'s signature algorithm names, for `key`'s curve where
/// it is an elliptic one.
fn algorithm(
    certificate: &X509Certificate<'_>,
    key: &SubjectPublicKeyInfo<'_>,
) -> Option<&'static dyn VerificationAlgorithm> {
    let named = &certificate.signature_algorithm.algorithm;
    let rsa: [(Oid<'_>, &'static dyn VerificationAlgorithm); 3] = [
        (OID_PKCS1_SHA256WITHRSA, &RSA_PKCS1_2048_8192_SHA256),
        (OID_PKCS1_SHA384WITHRSA, &RSA_PKCS1_2048_8192_SHA384),
        (OID_PKCS1_SHA512WITHRSA, &RSA_PKCS1_2048_8192_SHA512),
    ];
    let elliptic: [(Oid<'_>, Oid<'_>, &'static dyn VerificationAlgorithm); 4] = [
        (
            OID_EC_P256,
            OID_SIG_ECDSA_WITH_SHA256,
            &ECDSA_P256_SHA256_ASN1,
        ),
        (
            OID_EC_P256,
            OID_SIG_ECDSA_WITH_SHA384,
            &ECDSA_P256_SHA384_ASN1,
        ),
        (
            OID_NIST_EC_P384,
            OID_SIG_ECDSA_WITH_SHA256,
            &ECDSA_P384_SHA256_ASN1,
        ),
        (
            OID_NIST_EC_P384,
            OID_SIG_ECDSA_WITH_SHA384,
            &ECDSA_P384_SHA384_ASN1,
        ),
    ];
    let curve = key
        .algorithm
        .parameters
        .as_ref()
        .and_then(|parameters| parameters.as_oid().ok());
    rsa.into_iter()
        .find(|(signature, _)| signature == named)
        .map(|(_, verifying)| verifying)
        .or_else(|| {
            elliptic
                .into_iter()
                .find(|(on, signature, _)| Some(on) == curve.as_ref() && signature == named)
                .map(|(_, _, verifying)| verifying)
        })
}

/// `now` as a certificate writes a moment, or nothing before 1970 or past what it holds.
fn moment(now: SystemTime) -> Option<ASN1Time> {
    let seconds = now.duration_since(UNIX_EPOCH).ok()?.as_secs();
    ASN1Time::from_timestamp(i64::try_from(seconds).ok()?).ok()
}

#[cfg(test)]
mod tests;
