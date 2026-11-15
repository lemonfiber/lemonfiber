//! A phone's device key registered, listed and challenged, from a recorded attestation.

use base64::Engine as _;
use lemonfiber_core::device_keys::identity::SETTING;
use lemonfiber_fixtures::attestations::ANDROID_RSA_ROOT;
use lemonfiber_fixtures::passwords::a_password;
use serde_json::{json, Value};

use crate::surface::Surface;
use crate::world::World;
use crate::Seen;

/// The name the phone's key is registered under.
const PHONE: &str = "pixel";

/// What a refusal of an untrusted app opens with, before the app it names.
const UNTRUSTED: &str = "This stack does not trust ";

/// `bytes` in base64url, unpadded.
fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// The `SubjectPublicKeyInfo` a DER certificate carries: its first sequence whose
/// algorithm is an elliptic-curve key.
fn spki_of(certificate: &[u8]) -> &[u8] {
    let marker = [
        0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01,
    ];
    certificate
        .windows(marker.len())
        .position(|window| window == marker)
        .and_then(|at| certificate.get(at..at + 91))
        .unwrap_or_default()
}

/// What the phone sends to register its key.
fn registering() -> Value {
    let chain: Vec<String> = ANDROID_RSA_ROOT.chain.iter().map(|der| b64(der)).collect();
    let own = ANDROID_RSA_ROOT.chain.first().copied().unwrap_or_default();
    json!({
        "name": PHONE,
        "public_key": b64(spki_of(own)),
        "attestation": { "platform": "android", "chain": chain },
        "password": a_password(),
    })
}

/// The phone registered once the stack trusts the app the first attempt names, its key
/// listed, and a challenge issued to it, and what each answered kept.
pub(crate) async fn every(seen: &mut Seen) -> Option<()> {
    let world = World::named("device-keys").at(ANDROID_RSA_ROOT.at);
    let surface = Surface::over(world).await?;
    let refused = surface
        .asked("POST", "/api/device-keys", Some(&registering()))
        .await;
    let named = refused
        .envelopes
        .first()
        .and_then(|envelope| envelope.pointer("/data/summary"))
        .and_then(Value::as_str)
        .and_then(|summary| summary.strip_prefix(UNTRUSTED))
        .unwrap_or_default()
        .to_owned();
    surface.world().trusting(SETTING, &named);
    for (method, path, body) in [
        ("POST", "/api/device-keys".to_owned(), Some(registering())),
        ("GET", "/api/device-keys".to_owned(), None),
        ("POST", format!("/api/device-keys/{PHONE}/challenge"), None),
    ] {
        seen.answered(surface.asked(method, &path, body.as_ref()).await.envelopes);
    }
    Some(())
}
