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
mod tests;
