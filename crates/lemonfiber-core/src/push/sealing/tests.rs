use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ring::agreement::{self, EphemeralPrivateKey, UnparsedPublicKey, ECDH_P256};
use ring::rand::{self, SystemRandom};
use ring::{aead, hkdf};

use super::{composed, readable, sealed, Long, Parties, WAKE};

/// Base64url, read for a test.
fn read(written: &str) -> Vec<u8> {
    URL_SAFE_NO_PAD.decode(written).unwrap_or_default()
}

/// The worked example the RFC gives comes out byte for byte.
#[test]
fn the_rfcs_worked_example_comes_out_byte_for_byte() {
    let theirs = read(concat!(
        "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkN",
        "toIAiw4",
    ));
    let ours = read(
        "BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8",
    );
    let salt: [u8; 16] = read("DGv6ra1nlYgDCS1FRnbzlw")
        .try_into()
        .unwrap_or_default();
    let coded = composed(
        &Parties {
            shared: &read("kyrL1jIIOHEzg3sM2ZWRHDRB62YACZhhSlknJ672kSs"),
            theirs: &theirs,
            ours: &ours,
            secret: &read("BTBZMqHH6r4Tts7J_aSIgg"),
            salt: &salt,
        },
        b"When I grow up, I want to be a watermelon",
    );
    assert_eq!(
        coded.map(|coded| URL_SAFE_NO_PAD.encode(coded)).as_deref(),
        Some(
            "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLo\
             cInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBW\
             LVWGNWQexSgSxsj_Qulcy4a-fN"
        )
    );
}

/// `long` bytes derived as the phone derives them.
fn derived(salt: &[u8], keying: &[u8], info: &[u8], long: usize) -> Vec<u8> {
    let mut out = vec![0; long];
    let filled = hkdf::Salt::new(hkdf::HKDF_SHA256, salt)
        .extract(keying)
        .expand(&[info], Long(long))
        .and_then(|expanded| expanded.fill(&mut out));
    assert!(filled.is_ok());
    out
}

/// A phone holding the keys reads what was sealed to them, and it says only that there is
/// something to say.
#[test]
fn a_phone_reads_what_was_sealed_to_it_and_it_says_only_to_wake() {
    let random = SystemRandom::new();
    let phone = EphemeralPrivateKey::generate(&ECDH_P256, &random);
    let public = phone
        .as_ref()
        .ok()
        .and_then(|phone| phone.compute_public_key().ok());
    assert!(public.is_some(), "a key was made");
    let (Ok(phone), Some(public)) = (phone, public) else {
        return;
    };
    let secret = rand::generate::<[u8; 16]>(&random)
        .map(rand::Random::expose)
        .unwrap_or_default();
    let coded = sealed(public.as_ref(), &secret).unwrap_or_default();

    let salt = coded.get(..16).unwrap_or_default();
    let ours = coded.get(21..86).unwrap_or_default();
    let shared =
        agreement::agree_ephemeral(phone, &UnparsedPublicKey::new(&ECDH_P256, ours), |shared| {
            shared.to_vec()
        })
        .unwrap_or_default();
    let info = [b"WebPush: info\0".as_slice(), public.as_ref(), ours].concat();
    let keying = derived(&secret, &shared, &info, 32);
    let key = derived(salt, &keying, b"Content-Encoding: aes128gcm\0", 16);
    let nonce: [u8; 12] = derived(salt, &keying, b"Content-Encoding: nonce\0", 12)
        .try_into()
        .unwrap_or_default();
    let mut body = coded.get(86..).unwrap_or_default().to_vec();
    let opening = aead::UnboundKey::new(&aead::AES_128_GCM, &key).map(aead::LessSafeKey::new);
    let opened = opening.and_then(|opening| {
        opening
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::empty(),
                &mut body,
            )
            .map(|opened| opened.to_vec())
    });
    assert_eq!(opened.ok(), Some([WAKE, &[2]].concat()));
}

/// Keys that cannot be encrypted to are not readable.
#[test]
fn keys_that_cannot_be_encrypted_to_are_not_readable() {
    let auth = URL_SAFE_NO_PAD.encode([1_u8; 16]);
    let not_on_the_curve = URL_SAFE_NO_PAD.encode([4_u8; 65]);
    assert!(readable(&not_on_the_curve, &auth).is_none());
    assert!(readable("not base64url!", &auth).is_none());
    assert!(readable(&URL_SAFE_NO_PAD.encode([4_u8; 12]), &auth).is_none());

    let random = SystemRandom::new();
    let public = EphemeralPrivateKey::generate(&ECDH_P256, &random)
        .and_then(|key| key.compute_public_key())
        .map(|public| URL_SAFE_NO_PAD.encode(public.as_ref()))
        .unwrap_or_default();
    assert!(readable(&public, &auth).is_some());
    assert!(readable(&public, "not base64url!").is_none());
    assert!(readable(&public, &URL_SAFE_NO_PAD.encode([1_u8; 4])).is_none());
}
