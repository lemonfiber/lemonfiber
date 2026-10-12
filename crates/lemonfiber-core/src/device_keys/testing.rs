//! Attestations made against test roots, for the refusals no recorded phone would show.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rcgen::{
    BasicConstraints, CertificateParams, CustomExtension, DistinguishedName, DnType, IsCa, Issuer,
    KeyPair, PublicKeyData, SigningKey, PKCS_ECDSA_P256_SHA256,
};

use super::identity::AppIdentity;
use super::PublicKey;

/// A moment every generated certificate is valid at.
pub(crate) fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

/// A moment a recorded attestation is valid at.
pub(crate) fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

/// A fresh P-256 key pair.
pub(crate) fn pair() -> KeyPair {
    KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap_or_else(|_| unreachable!())
}

/// The public half of `pair`, as a device key reads it.
pub(crate) fn public(pair: &KeyPair) -> PublicKey {
    PublicKey::from_der(&pair.subject_public_key_info()).unwrap_or_else(|| unreachable!())
}

/// `pair`'s signature over `message`, ECDSA in DER.
pub(crate) fn signed(pair: &KeyPair, message: &[u8]) -> Vec<u8> {
    pair.sign(message).unwrap_or_else(|_| unreachable!())
}

/// One DER value: `tag` as its identifier octets, then the length and `content`.
pub(crate) fn tlv(tag: &[u8], content: &[u8]) -> Vec<u8> {
    let length = content.len();
    let mut out = tag.to_vec();
    if length < 0x80 {
        out.push(u8::try_from(length).unwrap_or_default());
    } else {
        let bytes: Vec<u8> = length
            .to_be_bytes()
            .into_iter()
            .skip_while(|byte| *byte == 0)
            .collect();
        out.push(0x80 | u8::try_from(bytes.len()).unwrap_or_default());
        out.extend(bytes);
    }
    out.extend_from_slice(content);
    out
}

/// A sequence of `parts`.
pub(crate) fn sequence(parts: &[Vec<u8>]) -> Vec<u8> {
    tlv(&[0x30], &parts.concat())
}

/// A key description saying the attestation was made at `attested` and the key lives at
/// `held`, from an app with `packages` signed by `signers`.
pub(crate) fn key_description(
    attested: u8,
    held: u8,
    packages: &[&str],
    signers: &[[u8; 32]],
) -> Vec<u8> {
    let infos: Vec<Vec<u8>> = packages
        .iter()
        .map(|package| sequence(&[tlv(&[0x04], package.as_bytes()), tlv(&[0x02], &[1])]))
        .collect();
    let digests: Vec<Vec<u8>> = signers.iter().map(|signer| tlv(&[0x04], signer)).collect();
    let application = sequence(&[
        tlv(&[0x31], &infos.concat()),
        tlv(&[0x31], &digests.concat()),
    ]);
    let tagged = tlv(&[0xbf, 0x85, 0x45], &tlv(&[0x04], &application));
    sequence(&[
        tlv(&[0x02], &[4]),
        tlv(&[0x0a], &[attested]),
        tlv(&[0x02], &[4]),
        tlv(&[0x0a], &[held]),
        tlv(&[0x04], b"challenge"),
        tlv(&[0x04], b""),
        sequence(&[tagged]),
        sequence(&[]),
    ])
}

/// An authority: its certificate, and what it signs others with.
pub(crate) struct Authority {
    /// Its certificate's DER.
    pub(crate) der: Vec<u8>,
    /// What it signs with.
    issuer: Issuer<'static, KeyPair>,
}

/// The parameters of a certificate named `name`.
fn named(name: &str, ca: Option<BasicConstraints>) -> CertificateParams {
    let mut params =
        CertificateParams::new(Vec::<String>::new()).unwrap_or_else(|_| unreachable!());
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, name);
    params.distinguished_name = dn;
    params.is_ca = ca.map_or(IsCa::ExplicitNoCa, IsCa::Ca);
    params
}

impl Authority {
    /// A self-signed root named `name`.
    pub(crate) fn root(name: &str) -> Self {
        let params = named(name, Some(BasicConstraints::Unconstrained));
        let key = pair();
        let der = params
            .self_signed(&key)
            .unwrap_or_else(|_| unreachable!())
            .der()
            .to_vec();
        Self {
            der,
            issuer: Issuer::new(params, key),
        }
    }

    /// An authority named `name` this one signs, a certificate authority where `ca`.
    pub(crate) fn below(&self, name: &str, ca: Option<BasicConstraints>) -> Self {
        let params = named(name, ca);
        let key = pair();
        let der = params
            .signed_by(&key, &self.issuer)
            .unwrap_or_else(|_| unreachable!())
            .der()
            .to_vec();
        Self {
            der,
            issuer: Issuer::new(params, key),
        }
    }

    /// A certificate named `name` for `key`, this authority's, carrying `extensions`.
    pub(crate) fn certify(
        &self,
        name: &str,
        key: &impl PublicKeyData,
        extensions: Vec<CustomExtension>,
    ) -> Vec<u8> {
        let mut params = named(name, None);
        params.custom_extensions = extensions;
        params
            .signed_by(key, &self.issuer)
            .unwrap_or_else(|_| unreachable!())
            .der()
            .to_vec()
    }
}

/// The parts of an OID written in dotted form.
pub(crate) fn arcs(oid: &str) -> Vec<u64> {
    oid.split('.').filter_map(|arc| arc.parse().ok()).collect()
}

/// An Android identity for `package`, signed by `signer`.
pub(crate) fn android(package: &str, signer: [u8; 32]) -> AppIdentity {
    AppIdentity::Android {
        package: package.to_owned().into(),
        certificate: crate::secret::render(&signer).into(),
    }
}
