use super::{Mint, CLAIM, FORM, KEY};
use crate::device_keys::encoded;
use crate::device_keys::testing::{pair, public, signed};

/// A mint of a `read` key for Home Assistant, as a claim.
const MINT: Mint<'static> = Mint {
    name: "home-assistant",
    scope: "read",
    purpose: "home-assistant",
    claim: true,
};

#[test]
fn the_signed_text_is_the_form_the_challenge_and_the_mint_one_to_a_line() {
    assert_eq!(
        MINT.signed("c"),
        format!("{FORM}\nc\nhome-assistant\nread\nhome-assistant\n{CLAIM}\n")
    );
    let key = Mint {
        claim: false,
        ..MINT
    };
    assert!(key.signed("c").ends_with(&format!("\n{KEY}\n")));
}

#[test]
fn a_signature_proves_the_mint_it_was_made_for_and_no_other() {
    let pair = pair();
    let key = public(&pair);
    let signature = encoded(&signed(&pair, MINT.signed("challenge").as_bytes()));
    assert!(MINT.proved("challenge", &signature, &key));
    assert!(!MINT.proved("another", &signature, &key));
    for other in [
        Mint {
            name: "mcp",
            ..MINT
        },
        Mint {
            scope: "act",
            ..MINT
        },
        Mint {
            purpose: "other",
            ..MINT
        },
        Mint {
            claim: false,
            ..MINT
        },
    ] {
        assert!(!other.proved("challenge", &signature, &key));
    }
    assert!(!MINT.proved(
        "challenge",
        &signature,
        &public(&crate::device_keys::testing::pair())
    ));
    assert!(!MINT.proved("challenge", "not base64url!", &key));
}

#[test]
fn a_part_holding_a_line_end_is_proved_by_nothing() {
    let pair = pair();
    let key = public(&pair);
    let smuggled = Mint {
        name: "a\nread",
        ..MINT
    };
    let signature = encoded(&signed(&pair, smuggled.signed("challenge").as_bytes()));
    assert!(!smuggled.proved("challenge", &signature, &key));
}
