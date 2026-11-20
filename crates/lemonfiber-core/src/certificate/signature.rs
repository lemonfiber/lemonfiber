//! Whether a certificate was signed by a key, under the algorithms this reads.

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
use x509_parser::prelude::X509Certificate;
use x509_parser::x509::SubjectPublicKeyInfo;

/// Whether `certificate`'s signature is `key`'s, under an algorithm this accepts.
///
/// RSA with SHA-256 or wider, and ECDSA on P-256 or P-384 with SHA-256 or SHA-384. SHA-1
/// and anything else is refused, whatever a certificate names.
pub(crate) fn signed_by(certificate: &X509Certificate<'_>, key: &SubjectPublicKeyInfo<'_>) -> bool {
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
