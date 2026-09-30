use lemonfiber_core::companion::{Material, Pairing, Replacement};

use super::{certificate, pairing};

/// Material as a phone would be handed it, from a machine answering to its own name.
fn made(caution: Option<&str>) -> Pairing {
    let material = Material {
        address: "https://den.local:8443".to_owned(),
        fingerprint: "ab".repeat(32),
        expires: 1_790_813_400,
        stack: "000102030405060708090a0b0c0d0e0f".to_owned(),
    };
    Pairing {
        written: serde_json::to_string(&material).unwrap_or_default(),
        compare: lemonfiber_core::companion::comparable(&material.fingerprint),
        material,
        until: "2026-10-01T00:10:00".to_owned(),
        replacing: "It changes only when somebody replaces it.".to_owned(),
        caution: caution.map(str::to_owned),
    }
}

/// The code comes first, then the same line to type, then what to check and know.
#[test]
fn pairing_leads_with_the_code_and_repeats_it_as_a_line_to_type() {
    let report = made(None);
    let said = pairing(&report).text();
    let code = said.find('\u{2588}').or_else(|| said.find("##"));
    let typed = said.find(&report.written);
    assert!(code.is_some() && code < typed, "{said}");
    for part in [
        "https://den.local:8443",
        report.material.fingerprint.as_str(),
        report.compare.as_str(),
        "2026-10-01T00:10:00 UTC, 10 minutes from now",
        "holds no password",
        "It changes only when somebody replaces it.",
        "Replacing it is `lemonfiber companion certificate --confirm`.",
    ] {
        assert!(said.contains(part), "{part} in {said}");
    }
    assert!(!said.contains("number"), "{said}");
}

/// An address that is a number says so beside it.
#[test]
fn a_numbered_address_is_cautioned_beside_it() {
    let said = pairing(&made(Some("That address is a number."))).text();
    assert!(said.contains("  That address is a number."), "{said}");
}

/// Unconfirmed, nothing is replaced and the cost comes first; confirmed, what a phone
/// pins from now on is named.
#[test]
fn replacing_says_its_cost_before_and_its_new_certificate_after() {
    let kept = Some("ab".repeat(32));
    let asked = certificate(&Replacement {
        replaced: false,
        fingerprint: kept.clone(),
        consequence: "Every phone refuses it.".to_owned(),
    })
    .text();
    assert!(asked.starts_with("Nothing was replaced."), "{asked}");
    assert!(
        asked.contains("Replacing it: Every phone refuses it."),
        "{asked}"
    );
    assert!(asked.contains("companion certificate --confirm"), "{asked}");

    let done = certificate(&Replacement {
        replaced: true,
        fingerprint: kept,
        consequence: "Every phone refuses it.".to_owned(),
    })
    .text();
    assert!(done.starts_with("The certificate was replaced."), "{done}");
    assert!(done.contains("A phone paired from now on pins"), "{done}");

    let none = certificate(&Replacement {
        replaced: false,
        fingerprint: None,
        consequence: "Every phone refuses it.".to_owned(),
    })
    .text();
    assert!(none.contains("None has been made yet"), "{none}");
}

/// Both answers reach their renderer from the one place every outcome is shaped.
#[test]
fn both_answers_are_shaped_by_their_own_renderer() {
    let report = made(None);
    assert_eq!(
        crate::render::shaped(&lemonfiber_core::app::Outcome::Pairing(report.clone())).text(),
        pairing(&report).text()
    );
    let replaced = Replacement {
        replaced: true,
        fingerprint: Some("ab".repeat(32)),
        consequence: "Every phone refuses it.".to_owned(),
    };
    assert_eq!(
        crate::render::shaped(&lemonfiber_core::app::Outcome::Certificate(
            replaced.clone()
        ))
        .text(),
        certificate(&replaced).text()
    );
}
