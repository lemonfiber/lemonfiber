use lemonfiber_ports::registry::{Image, Offered, Unanswerable};
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};

use super::{held, Key, Provenance, Vouched, P256_SPKI_PREFIX};

/// The image every case here is about.
fn komga() -> Image {
    Image::new(
        "docker.io/gotson/komga",
        "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
    )
}

/// What a registry serves as the thing that was signed.
fn payload(about: &str) -> String {
    format!(
        r#"{{"critical":{{"identity":{{"docker-reference":"docker.io/gotson/komga"}},"image":{{"docker-manifest-digest":"{about}"}},"type":"cosign container image signature"}},"optional":null}}"#
    )
}

/// The reason an answer gives, for the two that carry one.
///
/// A signed image has a key rather than a reason, so this says `None` for it —
/// which is a thing a case can assert, and better than a sentence invented here.
fn why(read: &Provenance) -> Option<&str> {
    match read {
        Provenance::Signed { .. } => None,
        Provenance::Unproven { why } | Provenance::Refused { why } => Some(why),
    }
}

/// One real keypair, and a signature it really made.
///
/// Generated rather than pinned. A fixture signature is a fixture of somebody's
/// key, and the property worth holding is that this build can tell a signature
/// that was made over these bytes from one that was not — which needs a signer,
/// not a recording of one.
///
/// `None` where this machine could not produce one at all. Every case below
/// asserts against what it built, so a signer that failed fails the case rather
/// than leaving it to pass over nothing.
fn signed(over: &str) -> Option<(Key, Vec<u8>)> {
    let rng = SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).ok()?;
    let pair =
        EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng).ok()?;
    let signature = pair.sign(&rng, over.as_bytes()).ok()?.as_ref().to_vec();
    let key = as_key("the operator's own", pair.public_key().as_ref())?;
    Some((key, signature))
}

/// The PEM an operator would hold, built around a point.
fn as_key(named: &str, point: &[u8]) -> Option<Key> {
    use base64::Engine as _;
    let mut der = P256_SPKI_PREFIX.to_vec();
    der.extend_from_slice(point);
    let body = base64::engine::general_purpose::STANDARD.encode(&der);
    let pem = format!("-----BEGIN PUBLIC KEY-----\n{body}\n-----END PUBLIC KEY-----\n");
    Key::from_pem(named, &pem).ok()
}

/// One signature offered over one payload, as a registry would hand it over.
fn offering(payload: &str, signature: Vec<u8>) -> Vec<Offered> {
    vec![Offered {
        payload: payload.to_owned(),
        signature,
    }]
}

/// A signature over a payload about this image, made by a key we hold.
#[test]
fn a_signature_a_held_key_made_over_this_image_is_the_one_answer_that_says_so() {
    let about = payload(&komga().digest);
    let read = signed(&about)
        .map(|(key, signature)| held(&komga(), &Ok(offering(&about, signature)), &[key]));

    assert_eq!(
        read,
        Some(Provenance::Signed {
            by: "the operator's own".to_owned()
        })
    );
    assert!(read.as_ref().is_some_and(Provenance::is_signed));
    assert!(!read.as_ref().is_some_and(Provenance::refuses));
    assert_eq!(read.as_ref().map(Provenance::as_str), Some("signed"));
    assert_eq!(read.as_ref().and_then(why), None);
}

/// The floor: every way of not knowing lands on unproven, and none on signed.
///
/// This is the property the whole module is for. A verification that passed over
/// an answer it never got would be a supply-chain check reporting that an image
/// is vouched for because nobody was home to say otherwise — the cheapest
/// mistake in this repository, in the most expensive place to make it.
#[test]
fn nothing_that_established_nothing_is_ever_reported_as_signed() {
    let about = payload(&komga().digest);
    let ways = signed(&about)
        .map(|(key, signature)| {
            let unreachable: Result<Vec<Offered>, Unanswerable> =
                Err(Unanswerable::about(&komga(), "connection refused"));
            let nothing: Result<Vec<Offered>, Unanswerable> = Ok(Vec::new());
            let unchecked: Result<Vec<Offered>, Unanswerable> = Ok(offering(&about, signature));
            vec![
                (
                    "a registry nobody could reach",
                    held(&komga(), &unreachable, &[key]),
                ),
                ("a registry offering nothing", held(&komga(), &nothing, &[])),
                (
                    "a signature and no key to check it",
                    held(&komga(), &unchecked, &[]),
                ),
            ]
        })
        .unwrap_or_default();

    assert_eq!(ways.len(), 3, "the signer these cases need was not built");
    for (what, read) in ways {
        assert!(
            !read.is_signed(),
            "{what} was read as a signed image: {read:?}"
        );
        assert!(
            !read.refuses(),
            "{what} refused an image nobody has said anything against: {read:?}"
        );
        assert_eq!(read.as_str(), "unproven", "{what}");
    }
}

/// Each of the three says which image, and why, in its own words.
#[test]
fn every_answer_names_the_image_it_is_about() {
    let nothing: Result<Vec<Offered>, Unanswerable> = Ok(Vec::new());
    let read = held(&komga(), &nothing, &[]);
    let said = why(&read).unwrap_or_default();

    assert_eq!(read.as_str(), "unproven");
    assert!(said.contains("docker.io/gotson/komga"), "{said}");
    assert!(said.contains(&komga().digest), "{said}");
    assert!(said.contains("made no claim"), "{said}");
}

/// A signature that does not verify is refused, not reported as unproven.
#[test]
fn a_signature_no_held_key_made_is_refused() {
    let about = payload(&komga().digest);
    let read = signed(&about).zip(signed("something else entirely")).map(
        |((_, signature), (somebody_else, _))| {
            held(&komga(), &Ok(offering(&about, signature)), &[somebody_else])
        },
    );

    assert!(read.as_ref().is_some_and(Provenance::refuses), "{read:?}");
    assert!(!read.as_ref().is_some_and(Provenance::is_signed));
    let said = read.as_ref().and_then(why).unwrap_or_default();
    assert!(said.contains("docker.io/gotson/komga"), "{said}");
    assert!(said.contains("no key this build holds made it"), "{said}");
}

/// A signature really made, about a different image, vouches for nothing here.
///
/// The check that needs no key at all, and the one that would let any signed
/// image stand in for any other if it were left out.
#[test]
fn a_signature_about_another_image_is_refused_naming_both_digests() {
    let elsewhere = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let about = payload(elsewhere);
    let read = signed(&about)
        .map(|(key, signature)| held(&komga(), &Ok(offering(&about, signature)), &[key]));

    assert!(read.as_ref().is_some_and(Provenance::refuses), "{read:?}");
    let said = read.as_ref().and_then(why).unwrap_or_default();
    assert!(said.contains(elsewhere), "names what it is about: {said}");
    assert!(said.contains(&komga().digest), "and what it is not: {said}");
}

/// A payload that is not a signing document at all is refused, not guessed at.
#[test]
fn a_payload_that_is_not_a_signing_document_is_refused() {
    let reads: Vec<(&str, Option<Provenance>)> = ["not json at all", r#"{"critical":{}}"#]
        .into_iter()
        .map(|nonsense| {
            let read = signed(nonsense)
                .map(|(key, signature)| held(&komga(), &Ok(offering(nonsense, signature)), &[key]));
            (nonsense, read)
        })
        .collect();

    assert_eq!(reads.len(), 2, "no payload was put in front of this");
    for (nonsense, read) in reads {
        assert!(
            read.as_ref().is_some_and(Provenance::refuses),
            "{nonsense} was not refused: {read:?}"
        );
    }
}

/// Several offered, one of which holds, is a signed image.
#[test]
fn one_signature_that_holds_among_several_is_enough() {
    let about = payload(&komga().digest);
    let read =
        signed(&about)
            .zip(signed("something else"))
            .map(|((key, signature), (_, wrong))| {
                let answer = Ok(vec![
                    Offered {
                        payload: about.clone(),
                        signature: wrong,
                    },
                    Offered {
                        payload: about.clone(),
                        signature,
                    },
                ]);
                held(&komga(), &answer, &[key])
            });

    assert!(read.as_ref().is_some_and(Provenance::is_signed), "{read:?}");
}

/// A registry with one answer, so a whole plugin can be asked about.
///
/// One rather than a script: every case here pins a single image, and a script
/// needs an arm for running out that no case would ever reach.
struct Answering(Result<Vec<Offered>, Unanswerable>);

#[async_trait::async_trait]
impl lemonfiber_ports::registry::Registry for Answering {
    async fn signatures(&self, _image: &Image) -> Result<Vec<Offered>, Unanswerable> {
        self.0.clone()
    }
}

/// A manifest pinning one image, in the shape a plugin's source takes.
fn manifest() -> Option<lemonfiber_plugin::Manifest> {
    let text = format!(
        r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://github.com/gotson/komga"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "docker.io/gotson/komga"
digest      = "{}"
tag         = "1.11.0"
criticality = "important"
"#,
        komga().digest
    );
    lemonfiber_plugin::Manifest::from_toml(&text).ok()
}

/// The fixture manifest read against a scripted registry, after a case shapes it.
///
/// `None` only where the fixture itself stopped reading. Each case asserts on
/// what came back rather than on an unwrapped value, so a fixture that broke
/// fails the case instead of leaving it to pass over nothing.
async fn read(
    shape: impl FnOnce(&mut lemonfiber_plugin::Manifest),
    keys: &[Key],
    asking: &Answering,
) -> Option<Vouched> {
    let mut one = manifest()?;
    shape(&mut one);
    Some(super::vouched(&one, keys, asking).await)
}

#[tokio::test]
async fn an_image_nobody_signed_does_not_stop_an_install() {
    let asking = Answering(Ok(Vec::new()));
    let read = read(|_| (), &[], &asking).await;

    assert_eq!(
        read.as_ref().map(|one| one.installable),
        Some(true),
        "an unsigned image refused an install: {read:?}"
    );
    assert_eq!(read.as_ref().map(|one| one.images.len()), Some(1));
    assert_eq!(
        read.as_ref()
            .and_then(|one| one.images.first())
            .map(|one| one.held.as_str()),
        Some("unproven")
    );
}

/// The fixture plugin read against a registry offering one signature we made.
async fn offered_for(about: &str) -> Option<Vouched> {
    let (key, signature) = signed(about)?;
    let asking = Answering(Ok(offering(about, signature)));
    read(|_| (), &[key], &asking).await
}

#[tokio::test]
async fn an_image_whose_signature_does_not_hold_stops_one() {
    let about = payload("sha256:0000000000000000000000000000000000000000000000000000000000000000");
    let asked = offered_for(&about).await;

    assert_eq!(
        asked.as_ref().map(|one| one.installable),
        Some(false),
        "a refused image did not stop an install: {asked:?}"
    );
}

/// A plugin pinning nothing has had nothing established about it.
///
/// The floor at the other end: a read over no image at all would otherwise report
/// that nothing said about these images stops an install, which is true and about
/// nothing.
#[tokio::test]
async fn a_plugin_that_pins_no_image_is_not_read_as_one_nobody_objects_to() {
    let asking = Answering(Ok(Vec::new()));
    let read = read(|one| one.services.clear(), &[], &asking).await;

    assert_eq!(read.as_ref().map(|one| one.images.is_empty()), Some(true));
    assert_eq!(
        read.as_ref().map(|one| one.installable),
        Some(false),
        "a plugin pinning no image was read as one nobody objects to: {read:?}"
    );
}

/// A key this build cannot read is told so, rather than read as sixty-five bytes.
#[test]
fn a_key_that_is_not_a_p256_public_key_is_refused_by_name() {
    use base64::Engine as _;
    let short = base64::engine::general_purpose::STANDARD.encode([0_u8; 8]);
    for (what, pem) in [
        (
            "nothing between the markers",
            "-----BEGIN PUBLIC KEY-----\n-----END PUBLIC KEY-----\n".to_owned(),
        ),
        (
            "not base64",
            "-----BEGIN PUBLIC KEY-----\n!!!!\n-----END PUBLIC KEY-----\n".to_owned(),
        ),
        (
            "the wrong wrapper",
            format!("-----BEGIN PUBLIC KEY-----\n{short}\n-----END PUBLIC KEY-----\n"),
        ),
    ] {
        let refused = Key::from_pem("theirs", &pem);
        assert!(refused.is_err(), "{what} was read as a key");
        let said = refused.err().map(|one| one.to_string()).unwrap_or_default();
        assert!(said.contains("theirs"), "{what}: {said}");
        assert!(said.contains("P-256"), "{what}: {said}");
    }
}
