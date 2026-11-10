//! Encrypting a message to a phone's `WebPush` keys, as RFC 8291 has it, in the `aes128gcm`
//! content coding of RFC 8188.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ring::agreement::{self, EphemeralPrivateKey, UnparsedPublicKey, ECDH_P256};
use ring::rand::{self, SystemRandom};
use ring::{aead, hkdf};

/// The record size the coding declares, which this one short message is within.
const RECORD_SIZE: u32 = 4096;

/// How long an uncompressed P-256 public key is.
const PUBLIC_KEY: usize = 65;

/// How long a `WebPush` secret is.
const SECRET: usize = 16;

/// The delimiter that ends the last record, and so the padding.
const LAST_RECORD: u8 = 2;

/// A length asked of the key derivation.
struct Long(usize);

impl hkdf::KeyType for Long {
    fn len(&self) -> usize {
        self.0
    }
}

/// `p256dh` and `auth` as the phone gave them, where they can be encrypted to.
pub(super) fn readable(p256dh: &str, auth: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let public = URL_SAFE_NO_PAD.decode(p256dh).ok()?;
    let secret = URL_SAFE_NO_PAD.decode(auth).ok()?;
    (public.len() == PUBLIC_KEY && secret.len() == SECRET && sealed(&public, &secret, "").is_some())
        .then_some((public, secret))
}

/// The message that wakes a phone holding `public` and `secret`, under a key made for it
/// alone: only that the stack has something to say, and the id the phone confirms it by.
pub(super) fn sealed(public: &[u8], secret: &[u8], id: &str) -> Option<Vec<u8>> {
    let wake = serde_json::json!({ "wake": true, "id": id }).to_string();
    let random = SystemRandom::new();
    let ours = EphemeralPrivateKey::generate(&ECDH_P256, &random).ok()?;
    let our_public = ours.compute_public_key().ok()?;
    let salt = rand::generate::<[u8; 16]>(&random).ok()?.expose();
    let shared = agreement::agree_ephemeral(
        ours,
        &UnparsedPublicKey::new(&ECDH_P256, public),
        <[u8]>::to_vec,
    )
    .ok()?;
    composed(
        &Parties {
            shared: &shared,
            theirs: public,
            ours: our_public.as_ref(),
            secret,
            salt: &salt,
        },
        wake.as_bytes(),
    )
}

/// What the encryption is made from.
struct Parties<'a> {
    /// What the two public keys agreed.
    shared: &'a [u8],
    /// The phone's public key.
    theirs: &'a [u8],
    /// The key made for this message.
    ours: &'a [u8],
    /// The phone's secret.
    secret: &'a [u8],
    /// The salt made for this message.
    salt: &'a [u8; 16],
}

/// `message` encrypted between `parties`, headed as the content coding has it.
fn composed(parties: &Parties<'_>, message: &[u8]) -> Option<Vec<u8>> {
    let info = [b"WebPush: info\0".as_slice(), parties.theirs, parties.ours].concat();
    let keying = derived(parties.secret, parties.shared, &info, 32)?;
    let key = derived(parties.salt, &keying, b"Content-Encoding: aes128gcm\0", 16)?;
    let nonce: [u8; 12] = derived(parties.salt, &keying, b"Content-Encoding: nonce\0", 12)?
        .try_into()
        .ok()?;
    let sealing = aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_128_GCM, &key).ok()?);
    let mut body = [message, &[LAST_RECORD]].concat();
    sealing
        .seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::empty(),
            &mut body,
        )
        .ok()?;
    let mut coded = parties.salt.to_vec();
    coded.extend(RECORD_SIZE.to_be_bytes());
    coded.push(u8::try_from(parties.ours.len()).ok()?);
    coded.extend(parties.ours);
    coded.extend(body);
    Some(coded)
}

/// `long` bytes derived from `keying` under `salt` and `info`.
fn derived(salt: &[u8], keying: &[u8], info: &[u8], long: usize) -> Option<Vec<u8>> {
    let mut out = vec![0; long];
    hkdf::Salt::new(hkdf::HKDF_SHA256, salt)
        .extract(keying)
        .expand(&[info], Long(long))
        .ok()?
        .fill(&mut out)
        .ok()?;
    Some(out)
}

#[cfg(test)]
mod tests;
