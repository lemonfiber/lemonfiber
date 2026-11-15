//! The certificate a paired phone pins, made the same on every run from one key, and the
//! stack served encrypted on the network beside it. The front door presents the same
//! certificate where a world plays titles through it.

use base64::Engine as _;
use lemonfiber_core::companion::served::{self, Served};

use crate::world::World;

/// The prefix that makes 32 bytes an Ed25519 private key in PKCS #8.
const PKCS8_ED25519: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

/// The key's 32 bytes.
const SEED: [u8; 32] = [7; 32];

/// The name the certificate is made out to.
const NAMED: &str = "localhost";

/// The port the stack was served on.
const SERVED_ON: u16 = 8471;

/// `der` as a PEM block labelled `label`.
fn pem(label: &str, der: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    let lines: Vec<&str> = encoded
        .as_bytes()
        .chunks(64)
        .filter_map(|line| std::str::from_utf8(line).ok())
        .collect();
    format!(
        "-----BEGIN {label}-----\n{}\n-----END {label}-----\n",
        lines.join("\n")
    )
}

/// `world`'s certificate kept where pairing reads it, and its last serving recorded as
/// encrypted and offered to the network.
pub(crate) fn serving(world: &World) {
    let directory = world.root().join("state").join("companion");
    certified(&directory);
    let _ = served::record(
        &directory,
        Served {
            port: SERVED_ON,
            encrypted: true,
            network: true,
        },
    );
}

/// The certificate made the same on every run from one key, and its key, kept in
/// `directory` as a certificate lemonfiber made is kept.
pub(crate) fn certified(directory: &std::path::Path) {
    let _ = std::fs::create_dir_all(directory);
    let key = [PKCS8_ED25519.as_slice(), SEED.as_slice()].concat();
    let made = rcgen::KeyPair::try_from(key.as_slice()).and_then(|pair| {
        rcgen::CertificateParams::new(vec![NAMED.to_owned()])
            .and_then(|params| params.self_signed(&pair))
            .map(|certificate| (certificate, pair))
    });
    if let Ok((certificate, pair)) = made {
        let _ = std::fs::write(
            directory.join("certificate.pem"),
            pem("CERTIFICATE", certificate.der()),
        );
        let _ = std::fs::write(
            directory.join("key.pem"),
            pem("PRIVATE KEY", &pair.serialize_der()),
        );
    }
}
