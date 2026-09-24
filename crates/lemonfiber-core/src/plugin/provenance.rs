//! Whether anybody has said an image is theirs, and whether that holds.
//!
//! Three answers and never two. An image whose signature verifies is **signed**; one
//! nobody signed is **unproven**; one carrying a signature that does not hold is
//! **refused**. The middle answer is the one that has to keep its own name: a
//! publisher who never signed anything has made no claim, and a claim that did not
//! check out is a different fact — an operator deciding whether to proceed needs to
//! tell them apart, and collapsing the two into a boolean loses exactly that.
//!
//! **Nothing here reaches a registry.** What the registry said arrives as an answer
//! already given, so every way of not knowing can be put in front of this without a
//! network: an outage, a repository nobody may look in, an empty answer, a payload
//! about another image, a signature that does not verify, and no key to check one
//! against.
//!
//! **Unproven is the floor, and it is a floor rather than a default.** Every way of
//! failing to establish something lands there, and `Signed` is produced at exactly
//! one place — after a key has verified a signature over a payload naming this
//! digest. A verification that passed over an empty answer would be the worst
//! instance of the cheapest mistake in this repository, so the empty answer is
//! written out as its own arm rather than falling through anything.

use lemonfiber_ports::registry::{Image, Offered, Unanswerable};
use ring::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_ASN1};
use serde::Serialize;

/// What an SPKI-wrapped P-256 public key carries before the point itself.
///
/// Fixed for this curve, so the parse is a length and a prefix rather than a DER
/// reader. A key that is not this shape is refused naming what was expected, which
/// is a better answer than a partial parse of something else.
const P256_SPKI_PREFIX: &[u8] = &[
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a,
    0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
];

/// An uncompressed point: the tag byte and the two coordinates.
const P256_POINT: usize = 65;

/// What lemonfiber makes of an image's provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "provenance", rename_all = "lowercase")]
pub enum Provenance {
    /// A signature over a payload naming this digest verified against a held key.
    Signed {
        /// Which key it verified against, so the answer names what was trusted.
        by: String,
    },
    /// Nothing was established, and nothing follows from it either way.
    Unproven {
        /// What stopped it being established.
        why: String,
    },
    /// A signature is offered and it does not hold.
    Refused {
        /// Every way it does not.
        why: String,
    },
}

impl Provenance {
    /// Whether this is the one answer that says somebody vouched for the image.
    ///
    /// Read rather than recomputed by each surface: *unproven is not verified* is one
    /// rule, and a second copy of it is free to disagree with this one.
    #[must_use]
    pub const fn is_signed(&self) -> bool {
        matches!(self, Self::Signed { .. })
    }

    /// Whether this stops an install.
    #[must_use]
    pub const fn refuses(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }

    /// The word a report uses for it.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Signed { .. } => "signed",
            Self::Unproven { .. } => "unproven",
            Self::Refused { .. } => "refused",
        }
    }
}

/// One key an operator holds, and the name they know it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// What the operator calls it, which is what an answer names.
    pub named: String,
    /// The uncompressed point, as the curve carries it.
    point: Vec<u8>,
}

/// A key that could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{named} is not a PEM public key on the P-256 curve: {why}")]
pub struct Unusable {
    /// What the operator called it.
    pub named: String,
    /// What is wrong with it.
    pub why: String,
}

impl Key {
    /// One key, read from the PEM an operator holds.
    ///
    /// P-256 and nothing else, because that is the curve the signatures in question
    /// are made on, and a key on another curve would verify nothing while looking
    /// like it might. The wrapper around the point is fixed for this curve, so it is
    /// checked as a prefix — a key that is not this shape is told so, rather than
    /// having its last sixty-five bytes read as a point.
    ///
    /// # Errors
    ///
    /// Returns [`Unusable`] where the text is not a PEM block, is not base64, or
    /// does not carry a P-256 point.
    pub fn from_pem(named: &str, pem: &str) -> Result<Self, Unusable> {
        let refuse = |why: &str| Unusable {
            named: named.to_owned(),
            why: why.to_owned(),
        };
        let body: String = pem
            .lines()
            .filter(|line| !line.starts_with("-----"))
            .flat_map(str::chars)
            .filter(|letter| !letter.is_whitespace())
            .collect();
        if body.is_empty() {
            return Err(refuse("it carries no base64 between its markers"));
        }
        let mut der = decoded(&body).ok_or_else(|| refuse("its base64 does not decode"))?;
        if !der.starts_with(P256_SPKI_PREFIX) || der.len() != P256_SPKI_PREFIX.len() + P256_POINT {
            return Err(refuse(
                "its wrapper is not the one a P-256 public key carries",
            ));
        }
        Ok(Self {
            named: named.to_owned(),
            point: der.split_off(P256_SPKI_PREFIX.len()),
        })
    }

    /// Whether this key made this signature over these bytes.
    fn made(&self, signature: &[u8], over: &str) -> bool {
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, &self.point)
            .verify(over.as_bytes(), signature)
            .is_ok()
    }
}

/// Base64, as the wrappers around keys and signatures carry it.
fn decoded(text: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(text).ok()
}

/// What this build makes of what a registry answered about one image.
///
/// `answer` is the registry's, already given. `keys` is what the operator holds, and
/// an empty set is not an error: it is the honest reason a signature that is offered
/// cannot be checked, and it produces *unproven* rather than either of the other two.
#[must_use]
pub fn held(
    image: &Image,
    answer: &Result<Vec<Offered>, Unanswerable>,
    keys: &[Key],
) -> Provenance {
    let offered = match answer {
        Err(unanswerable) => {
            return Provenance::Unproven {
                why: format!(
                    "{unanswerable}; nothing about this image follows from a question \
                     nobody could put"
                ),
            }
        }
        Ok(offered) => offered,
    };

    if offered.is_empty() {
        return Provenance::Unproven {
            why: format!(
                "{} offers no signature for {}; its publisher has made no claim about \
                 it, which is not the same as one that did not hold",
                image.repository, image.digest
            ),
        };
    }

    if keys.is_empty() {
        return Provenance::Unproven {
            why: format!(
                "{} offers {} signature(s) for {} and this build holds no key to check \
                 one against, so nothing has been established",
                image.repository,
                offered.len(),
                image.digest
            ),
        };
    }

    let mut faults: Vec<String> = Vec::new();
    for (at, one) in offered.iter().enumerate() {
        if let Err(why) = about(&one.payload, &image.digest) {
            faults.push(format!("signature {at}: {why}"));
            continue;
        }
        if let Some(key) = keys
            .iter()
            .find(|key| key.made(&one.signature, &one.payload))
        {
            return Provenance::Signed {
                by: key.named.clone(),
            };
        }
        faults.push(format!(
            "signature {at}: no key this build holds made it — the keys tried were {}",
            keys.iter()
                .map(|key| key.named.as_str())
                .collect::<Vec<&str>>()
                .join(", ")
        ));
    }

    Provenance::Refused {
        why: format!(
            "{} claims a signature for {} that does not hold: {}",
            image.repository,
            image.digest,
            faults.join("; ")
        ),
    }
}

/// Whether a signing payload is about this exact image.
///
/// The first thing asked of a signature and the one that needs no key: a payload
/// naming another image is a signature somebody really made, about something else,
/// and taking it as this image's would let any signed image vouch for any other.
fn about(payload: &str, digest: &str) -> Result<(), String> {
    let read: serde_json::Value = serde_json::from_str(payload)
        .map_err(|why| format!("its payload is not readable as a signing document ({why})"))?;
    let named = read
        .get("critical")
        .and_then(|critical| critical.get("image"))
        .and_then(|image| image.get("docker-manifest-digest"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "its payload names no image digest".to_owned())?;
    if named != digest {
        return Err(format!(
            "its payload is about {named} rather than about {digest}"
        ));
    }
    Ok(())
}

/// What one of a plugin's images is vouched for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Vouch {
    /// The service the image runs as.
    pub service: String,
    /// The registry path, as the manifest declares it.
    pub image: String,
    /// The digest that fixes what runs.
    pub digest: String,
    /// What this build makes of it.
    #[serde(flatten)]
    pub held: Provenance,
}

/// Every image a plugin pins, and what anybody has said about each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Vouched {
    /// The plugin's id.
    pub id: String,
    /// One answer per image it pins.
    pub images: Vec<Vouch>,
    /// Whether what was found stops an install.
    ///
    /// An unproven image does not: a publisher who signed nothing has made no claim,
    /// and refusing every unsigned image would refuse most of the registry. A refused
    /// one does, because a claim that did not hold is worse than none.
    pub installable: bool,
}

/// What a plugin's images are vouched for, asked of a registry.
///
/// # Errors
///
/// [`Unasked`] where the source holds no manifest this build can read. Everything a
/// registry does or does not say is an answer inside [`Vouched`], never an error:
/// *nothing could be established* is a verdict this read exists to give.
pub async fn vouched(
    manifest: &lemonfiber_plugin::Manifest,
    keys: &[Key],
    asking: &dyn lemonfiber_ports::registry::Registry,
) -> Vouched {
    let mut images = Vec::new();
    for service in &manifest.services {
        let image = Image::new(&service.image, &service.digest);
        let answer = asking.signatures(&image).await;
        images.push(Vouch {
            service: service.id.clone(),
            image: service.image.clone(),
            digest: service.digest.clone(),
            held: held(&image, &answer, keys),
        });
    }
    // A plugin pinning no image is one this read has established nothing about, and
    // saying it may be installed would be answering a question nobody asked. The
    // format refuses a manifest with no service; a read that quietly passed one
    // would be the second opinion that disagrees.
    let installable = !images.is_empty() && !images.iter().any(|one| one.held.refuses());
    Vouched {
        id: manifest.plugin.id.clone(),
        images,
        installable,
    }
}

#[cfg(test)]
mod tests {
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
                let read = signed(nonsense).map(|(key, signature)| {
                    held(&komga(), &Ok(offering(nonsense, signature)), &[key])
                });
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
        let about =
            payload("sha256:0000000000000000000000000000000000000000000000000000000000000000");
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
}
