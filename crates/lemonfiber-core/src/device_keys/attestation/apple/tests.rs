use lemonfiber_fixtures::attestations::APPLE;
use minicbor::Encoder;
use rcgen::{BasicConstraints, CustomExtension, PublicKeyData};

use super::{attested, attested_under, read, verified, NONCE, ROOTS};
use crate::device_keys::attestation::{Attested, Unattested};
use crate::device_keys::identity::AppIdentity;
use crate::device_keys::testing::{arcs, at, now, pair, public, sequence, tlv, Authority};
use crate::device_keys::{Backing, PublicKey};

/// The App ID the generated attestations are made for.
const APP_ID: &str = "ABCDE12345.app.lemonfiber.companion";

/// The SHA-256 of `bytes`.
fn sha(bytes: &[u8]) -> Vec<u8> {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .to_vec()
}

/// The identity of the generated attestations' app.
fn ours() -> AppIdentity {
    AppIdentity::read(&format!("ios:{APP_ID}")).unwrap_or_else(|| unreachable!())
}

#[test]
fn a_recorded_attestation_reads_back_to_apples_root_and_names_its_app() {
    let Some(object) = read(APPLE.object) else {
        unreachable!("the recording reads");
    };
    let app = verified(&ROOTS, &object, &sha(APPLE.client_data), at(APPLE.at));
    assert_eq!(app, Some(sha(APPLE.app_id.as_bytes()).as_slice()));
}

#[test]
fn a_recorded_attestation_of_other_client_data_vouches_for_nothing() {
    let Some(object) = read(APPLE.object) else {
        return;
    };
    assert_eq!(
        verified(&ROOTS, &object, &sha(b"another"), at(APPLE.at)),
        None
    );
    assert_eq!(
        verified(&ROOTS, &object, &sha(APPLE.client_data), now()),
        None
    );
}

#[test]
fn a_recorded_attestation_does_not_cover_a_device_key_it_was_not_made_for() {
    let key = public(&pair());
    assert_eq!(
        attested(APPLE.object, &key, &[], at(APPLE.at)),
        Err(Unattested::Unverified)
    );
}

/// What a generated attestation is made of, each part able to be spoiled.
struct Made {
    /// The App ID hashed into the authenticator data.
    app_id: String,
    /// The counter.
    counter: [u8; 4],
    /// The environment named.
    environment: [u8; 16],
    /// Whether the credential id is the hash of the credential certificate's key.
    keyed: bool,
    /// Whether the nonce is made over the right client data.
    nonced: bool,
}

impl Made {
    /// An attestation as App Attest makes one in production.
    fn genuine() -> Self {
        Self {
            app_id: APP_ID.to_owned(),
            counter: [0; 4],
            environment: *b"appattest\0\0\0\0\0\0\0",
            keyed: true,
            nonced: true,
        }
    }

    /// The object made for `device`, under a test root, and that root.
    fn object(&self, device: &PublicKey) -> (Vec<u8>, Authority) {
        let root = Authority::root("Apple App Attestation Root CA");
        let authority = root.below(
            "Apple App Attestation CA 1",
            Some(BasicConstraints::Unconstrained),
        );
        let credential = pair();
        let key_id = sha(credential.der_bytes());
        let mut data = sha(self.app_id.as_bytes());
        data.push(0x40);
        data.extend(self.counter);
        data.extend(self.environment);
        data.extend([0, 32]);
        data.extend(if self.keyed { key_id } else { vec![0; 32] });
        let client = if self.nonced {
            sha(device.encoded())
        } else {
            sha(b"something else")
        };
        let nonce = sha(&[data.as_slice(), client.as_slice()].concat());
        let extension = sequence(&[tlv(&[0xa1], &tlv(&[0x04], &nonce))]);
        let certificate = authority.certify(
            "credential",
            &credential,
            vec![CustomExtension::from_oid_content(&arcs(NONCE), extension)],
        );
        (cbor(&[certificate, authority.der.clone()], &data), root)
    }
}

/// An attestation object holding `chain` and `data`.
fn cbor(chain: &[Vec<u8>], data: &[u8]) -> Vec<u8> {
    let mut encoder = Encoder::new(Vec::new());
    let written = encoder
        .map(3)
        .and_then(|e| e.str("fmt"))
        .and_then(|e| e.str("apple-appattest"))
        .and_then(|e| e.str("attStmt"))
        .and_then(|e| e.map(2))
        .and_then(|e| e.str("x5c"))
        .and_then(|e| e.array(2))
        .and_then(|e| chain.iter().try_fold(e, |e, one| e.bytes(one)))
        .and_then(|e| e.str("receipt"))
        .and_then(|e| e.bytes(b"receipt"))
        .and_then(|e| e.str("authData"))
        .and_then(|e| e.bytes(data));
    assert!(written.is_ok());
    encoder.into_writer()
}

/// What `made`, for a fresh device key, attests under its own root.
fn attests(made: &Made) -> Result<Attested, Unattested> {
    let device = public(&pair());
    let (object, root) = made.object(&device);
    attested_under(&[&root.der], &object, &device, &[ours()], now())
}

#[test]
fn a_genuine_attestation_of_the_device_keys_hash_records_the_enclave() {
    assert_eq!(
        attests(&Made::genuine()),
        Ok(Attested {
            backing: Backing::Enclave,
            app: ours()
        })
    );
}

#[test]
fn the_development_environment_is_accepted_beside_production() {
    let made = Made {
        environment: *b"appattestdevelop",
        ..Made::genuine()
    };
    assert!(attests(&made).is_ok());
}

#[test]
fn an_attestation_spoiled_anywhere_vouches_for_nothing() {
    for spoiled in [
        Made {
            counter: [0, 0, 0, 1],
            ..Made::genuine()
        },
        Made {
            environment: *b"somewhereelse\0\0\0",
            ..Made::genuine()
        },
        Made {
            keyed: false,
            ..Made::genuine()
        },
        Made {
            nonced: false,
            ..Made::genuine()
        },
    ] {
        assert_eq!(attests(&spoiled), Err(Unattested::Unverified));
    }
}

#[test]
fn an_app_the_stack_does_not_trust_is_refused_by_the_hash_of_its_id() {
    let made = Made {
        app_id: "ZZZZZ99999.com.example.other".to_owned(),
        ..Made::genuine()
    };
    let refused = attests(&made);
    let hashed = crate::secret::render(&sha(made.app_id.as_bytes()));
    assert!(
        matches!(&refused, Err(Unattested::Untrusted(named)) if named.contains(&hashed)),
        "{refused:?}"
    );
}

#[test]
fn a_generated_attestation_is_not_read_back_to_apples_root() {
    let device = public(&pair());
    let (object, _) = Made::genuine().object(&device);
    assert_eq!(
        attested(&object, &device, &[ours()], now()),
        Err(Unattested::Unverified)
    );
}

#[test]
fn only_an_app_attest_object_with_each_field_once_reads() {
    assert!(read(b"").is_none());
    assert!(read(b"\xa0").is_none());
    let mut encoder = Encoder::new(Vec::new());
    let twice = encoder
        .map(2)
        .and_then(|e| e.str("fmt"))
        .and_then(|e| e.str("apple-appattest"))
        .and_then(|e| e.str("fmt"))
        .and_then(|e| e.str("apple-appattest"));
    assert!(twice.is_ok());
    assert!(read(&encoder.into_writer()).is_none());
    let mut other = Encoder::new(Vec::new());
    let packed = other
        .map(1)
        .and_then(|e| e.str("fmt"))
        .and_then(|e| e.str("packed"));
    assert!(packed.is_ok());
    assert!(read(&other.into_writer()).is_none());
}
