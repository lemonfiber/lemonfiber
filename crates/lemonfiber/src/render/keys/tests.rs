use lemonfiber_core::keys::{Listed, Listing, Minted, Purpose, Secret, State};
use lemonfiber_fixtures::ports::Chance;

use super::{listing, minted};

/// A secret minted from a source a test chose.
fn a_secret() -> Secret {
    let Some(secret) = Secret::mint(&Chance::exactly(Some(vec![7; 32]))) else {
        unreachable!("thirty-two bytes mint a secret")
    };
    secret
}

/// A key just minted, with an address and a pin where `served`.
fn just_minted(served: bool) -> Minted {
    Minted {
        name: "home-assistant".to_owned(),
        scope: "read".to_owned(),
        purpose: Purpose::HomeAssistant,
        secret: a_secret(),
        address: served.then(|| "https://192.168.1.9:8443".to_owned()),
        pin: Some("ab".repeat(32)),
        caution: (!served).then(|| "Nothing has served this stack yet.".to_owned()),
    }
}

/// One key in a listing.
fn a_key(name: &str, state: State, member_minted: bool) -> Listed {
    Listed {
        name: name.to_owned(),
        scope: "act".to_owned(),
        purpose: Purpose::Other,
        state,
        minted: "2026-10-05T06:00:00".to_owned(),
        used: None,
        revoked: (state == State::Revoked).then(|| "2026-10-05T07:00:00".to_owned()),
        member_minted,
    }
}

#[test]
fn a_minted_key_is_shown_once_with_what_a_client_elsewhere_needs() {
    let report = just_minted(true);
    let said = minted(&report).text();
    assert!(
        said.contains("Minted the key home-assistant, with the scope read"),
        "{said}"
    );
    assert!(said.contains(report.secret.as_str()), "{said}");
    assert!(said.contains("not shown again"), "{said}");
    assert!(said.contains("The same secret, for a camera:"), "{said}");
    assert!(said.contains("address  https://192.168.1.9:8443"), "{said}");
    assert!(
        said.contains(&format!("pin      {}", "ab".repeat(32))),
        "{said}"
    );
}

#[test]
fn a_key_minted_before_anything_served_the_stack_says_how_to_serve_it() {
    let said = minted(&just_minted(false)).text();
    assert!(!said.contains("address"), "{said}");
    assert!(
        said.contains("Nothing has served this stack yet."),
        "{said}"
    );
}

#[test]
fn the_listing_names_each_key_and_where_it_stands() {
    let said = listing(&Listing {
        keys: vec![
            a_key("ha", State::Active, false),
            a_key("old", State::Revoked, false),
            a_key("anas", State::Orphaned, true),
            a_key("bens", State::Unconfirmed, true),
        ],
        revoked: None,
        purposes: "What a purpose is worth.".to_owned(),
        rehearsed: false,
    })
    .text();
    assert!(said.contains("  ha  act  other  active"), "{said}");
    assert!(said.contains("never used"), "{said}");
    assert!(said.contains("revoked 2026-10-05T07:00:00"), "{said}");
    assert!(said.contains("orphaned"), "{said}");
    assert!(said.contains("minted by the member"), "{said}");
    assert!(said.contains("could not be asked"), "{said}");
    assert!(said.contains("What a purpose is worth."), "{said}");
}

#[test]
fn a_revoke_says_what_it_did_or_would_do() {
    let report = |rehearsed: bool| Listing {
        keys: vec![a_key("ha", State::Revoked, false)],
        revoked: Some("ha".to_owned()),
        purposes: String::new(),
        rehearsed,
    };
    assert!(listing(&report(false))
        .text()
        .contains("Revoked the key ha."));
    assert!(listing(&report(true))
        .text()
        .contains("Revoking would refuse the key ha"));
}

#[test]
fn a_machine_with_no_keys_says_so() {
    let said = listing(&Listing {
        keys: Vec::new(),
        revoked: None,
        purposes: String::new(),
        rehearsed: false,
    })
    .text();
    assert!(said.contains("No key has been minted"), "{said}");
}

#[test]
fn a_key_in_use_says_when_it_was_last_used() {
    let mut key = a_key("ha", State::Active, false);
    key.used = Some("2026-10-05T08:00:00".to_owned());
    let said = listing(&Listing {
        keys: vec![key],
        revoked: None,
        purposes: "A purpose is what the minter said.".to_owned(),
        rehearsed: false,
    })
    .text();
    assert!(said.contains("last used 2026-10-05T08:00:00"), "{said}");
}
