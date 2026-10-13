//! Every refusal asking the device keys for something can meet, each in its own words.

use crate::error::codes::device::{
    ATTESTATION_REVOKED, ATTESTATION_UNVERIFIED, CHALLENGE_NOT_STANDING, DEVICE_NAME_TAKEN,
    IN_SOFTWARE, NOT_A_DEVICE_NAME, NOT_P256, NOT_REGISTERED, NO_ATTESTATION, NO_CHALLENGE,
    PUBLIC_HALF_TAKEN, SIGNATURE_WRONG, UNTRUSTED_APP,
};
use crate::error::{Diagnose, Problem, Remedy};
use crate::PRODUCT;

use super::super::identity::SETTING;
use super::super::{Record, Unattested, CURVE};
use crate::keys::LONGEST_NAME;

/// There is nowhere on this machine device keys are kept, or no password to keep them
/// under.
pub(super) fn nowhere() -> Problem {
    crate::config::store::Failure::Nowhere.problem()
}

/// The word cannot name a device key.
pub(super) fn bad_name(name: &str) -> Problem {
    Problem::new(
        NOT_A_DEVICE_NAME,
        format!("\"{name}\" cannot name a device key"),
        format!(
            "A device key's name travels in an address, an alert and the journal, so it is \
             lower-case letters, digits, dots, dashes and underscores, beginning with a letter \
             or a digit, and at most {LONGEST_NAME} characters."
        ),
        Remedy::new("Name it after the phone that holds it, as in pixel-8"),
    )
}

/// Another device key holds the name.
pub(super) fn name_taken(held: &Record) -> Problem {
    Problem::new(
        DEVICE_NAME_TAKEN,
        format!("A device key named {} is already registered", held.name),
        format!(
            "It was registered {}{}. A name identifies one device key for good, so an alert or \
             a journal entry naming it means the same key.",
            held.registered,
            standing(held)
        ),
        Remedy::new("Register it under another name"),
    )
}

/// Another device key holds the public half.
pub(super) fn key_taken(held: &Record) -> Problem {
    Problem::new(
        PUBLIC_HALF_TAKEN,
        "That public half is already registered",
        format!(
            "It is the device key named {}, registered {}{}. A revoked device key is never \
             made good again, so its public half is not registered a second time.",
            held.name,
            held.registered,
            standing(held)
        ),
        Remedy::new("Make a new key pair on the phone and register that"),
    )
}

/// The public half is not a P-256 key.
pub(super) fn not_p256() -> Problem {
    Problem::new(
        NOT_P256,
        format!("That public half is not a {CURVE} key"),
        format!(
            "A device key is the public half of a {CURVE} key pair, sent as its \
             SubjectPublicKeyInfo in base64url with its point uncompressed."
        ),
        Remedy::new(format!("Make the key pair on the {CURVE} curve")),
    )
}

/// No attestation came with the registration.
pub(super) fn no_attestation() -> Problem {
    Problem::new(
        NO_ATTESTATION,
        "Nothing vouches for where that key lives",
        "A device key is registered with the platform's attestation of where it lives, and a \
         key nothing vouches for is treated as one held in software. Nothing was registered.",
        Remedy::new("Send the platform's attestation of the key with it"),
    )
}

/// Why an attestation vouched for nothing.
pub(super) fn unproved(why: &Unattested) -> Problem {
    match why {
        Unattested::Unverified => Problem::new(
            ATTESTATION_UNVERIFIED,
            "That attestation does not vouch for this key",
            format!(
                "It does not verify against the platform roots this build of {PRODUCT} ships, or \
                 it does not cover the public half that was sent. Nothing was registered."
            ),
            Remedy::new(
                "Make a new key pair on the phone and register it with the attestation the \
                 platform gives for it",
            ),
        ),
        Unattested::InSoftware => Problem::new(
            IN_SOFTWARE,
            "That key was not made in the phone's hardware",
            "A device key has to be made and kept in the phone's hardware, StrongBox, the Secure \
             Enclave or the trusted environment, where the operating system cannot read it. A \
             key kept in software, or made outside the hardware and taken in, can be copied. \
             Nothing was registered.",
            Remedy::new("Mint the key at the stack's terminal instead")
                .with_detail(format!("{PRODUCT} key mint")),
        ),
        Unattested::Revoked => Problem::new(
            ATTESTATION_REVOKED,
            "Google has revoked what vouches for that key",
            format!(
                "A certificate in the chain vouching for it is one Google lists as revoked or \
                 suspended, in the snapshot of its attestation status list this build of \
                 {PRODUCT} carries, taken {}. A key it vouches for cannot be told from one made \
                 outside a phone's hardware. Nothing was registered.",
                super::super::attestation::status::taken().unwrap_or_default()
            ),
            Remedy::new("Mint the key at the stack's terminal instead")
                .with_detail(format!("{PRODUCT} key mint")),
        ),
        Unattested::Untrusted(identity) => Problem::new(
            UNTRUSTED_APP,
            format!("This stack does not trust {identity}"),
            format!(
                "A device key is registered only from the official companion or a build named \
                 in {SETTING}. Nothing was registered."
            ),
            Remedy::new("If it is a build you made, add its identity").with_detail(format!(
                "{PRODUCT} settings set {SETTING} <identities, comma-separated>"
            )),
        ),
    }
}

/// No active device key holds the name.
pub(super) fn not_registered(name: &str) -> Problem {
    Problem::new(
        NOT_REGISTERED,
        format!("No active device key is named {name}"),
        "None was registered under that name, or it was revoked. A phone whose key was \
         revoked registers a new one, which asks for the password once.",
        Remedy::new("Register the phone's key again"),
    )
}

/// The challenge a mint carried does not stand.
#[must_use]
pub fn challenge_not_standing() -> Problem {
    Problem::new(
        CHALLENGE_NOT_STANDING,
        "That challenge proves nothing",
        "It was never issued, it was used already, its minute has passed, or it was issued \
         to another session or device key. Nothing was minted.",
        Remedy::new("Ask for a new challenge and sign the mint within a minute"),
    )
}

/// The signature a mint carried does not verify.
#[must_use]
pub fn signature_wrong() -> Problem {
    Problem::new(
        SIGNATURE_WRONG,
        "That signature does not prove the operator",
        "It does not verify against the device key over that challenge and that mint. Nothing \
         was minted, and it counts against the limit wrong passwords meet.",
        Remedy::new(
            "Sign the challenge with the mint's name, scope, purpose and kind exactly as sent",
        ),
    )
}

/// This machine would not supply the bytes a challenge is made of.
#[must_use]
pub fn no_challenge() -> Problem {
    Problem::new(
        NO_CHALLENGE,
        format!("{PRODUCT} could not make a challenge"),
        "A challenge is unpredictable bytes this machine supplies, and it would not supply \
         them.",
        Remedy::new(
            "Try again, and if it happens twice the operating system's own random source is at \
             fault",
        ),
    )
}

/// How a device key stands, as a refusal naming it says.
fn standing(held: &Record) -> String {
    held.revoked
        .as_deref()
        .map_or_else(String::new, |when| format!(" and revoked {when}"))
}
