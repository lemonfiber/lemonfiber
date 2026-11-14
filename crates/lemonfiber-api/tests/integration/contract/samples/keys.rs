//! The keys a client mints, claims and registers, as the samples hold them.

/// A device key registered from an Android phone's `StrongBox`.
pub(super) fn a_device_key() -> lemonfiber_core::device_keys::Registered {
    lemonfiber_core::device_keys::Registered {
        name: "pixel".to_owned(),
        fingerprint: "cd".repeat(32),
        backing: lemonfiber_core::device_keys::Backing::Strongbox,
        app: format!("android:app.lemonfiber.companion@{}", "ef".repeat(32)),
        registered: lemonfiber_core::instant::Instant::from_seconds(1_760_248_800),
    }
}

/// One device key in use and one revoked, so every optional half is compared.
pub(super) fn every_device_key() -> lemonfiber_core::device_keys::Listing {
    use lemonfiber_core::device_keys::{Backing, Listed, Listing};
    let listed = |name: &str, backing: Backing| Listed {
        name: name.to_owned(),
        fingerprint: "cd".repeat(32),
        backing,
        app: "ios:ABCDE12345.app.lemonfiber.companion".to_owned(),
        registered: lemonfiber_core::instant::Instant::from_seconds(1_760_248_800),
        used: Some(lemonfiber_core::instant::Instant::from_seconds(
            1_760_252_400,
        )),
        revoked: None,
    };
    let mut gone = listed("old-phone", Backing::Tee);
    gone.revoked = Some(lemonfiber_core::instant::Instant::from_seconds(
        1_760_256_000,
    ));
    Listing {
        keys: vec![listed("iphone", Backing::Enclave), gone],
        revoked: Some("old-phone".to_owned()),
        rehearsed: false,
    }
}

/// A key just minted, with every optional half present: the address, the pin and a
/// caution beside them.
pub(super) fn a_minted_key() -> lemonfiber_core::keys::Minted {
    use lemonfiber_core::keys::{Minted, Purpose, Secret};
    let source = lemonfiber_fixtures::ports::Chance::exactly(Some(vec![7; 32]));
    let Some(secret) = Secret::mint(&source) else {
        unreachable!("thirty-two bytes mint a key")
    };
    Minted {
        name: "ha".to_owned(),
        scope: "act".to_owned(),
        purpose: Purpose::HomeAssistant,
        secret,
        address: Some("https://lemonfiber.local:8443".to_owned()),
        pin: Some("ab".repeat(32)),
        caution: Some("Serve it encrypted on your network first.".to_owned()),
    }
}

/// A key minted as a claim, with the link that collects it.
pub(super) fn a_key_claim() -> lemonfiber_core::keys::Claimed {
    lemonfiber_core::keys::Claimed {
        name: "ha".to_owned(),
        scope: "read".to_owned(),
        purpose: lemonfiber_core::keys::Purpose::HomeAssistant,
        link: format!(
            "lemonfiber://key?address=https%3A%2F%2Flemonfiber.local%3A8443&fingerprint={}&stack=a1b2&expires=1791245400&claim=lfc_{}",
            "ab".repeat(32),
            "07".repeat(32)
        ),
        lapses: lemonfiber_core::instant::Instant::from_seconds(1_791_245_400),
    }
}

/// A listing holding a key in use and one revoked, after a revoke.
pub(super) fn every_key() -> lemonfiber_core::keys::Listing {
    use lemonfiber_core::keys::{Listed, Listing, Purpose, State};
    let listed = |name: &str, state: State| Listed {
        name: name.to_owned(),
        scope: "read".to_owned(),
        purpose: Purpose::Mcp,
        state,
        minted: lemonfiber_core::instant::Instant::from_seconds(1_759_644_000),
        used: Some(lemonfiber_core::instant::Instant::from_seconds(
            1_759_647_600,
        )),
        revoked: None,
        member_minted: false,
        proved_by: Some("pixel".to_owned()),
    };
    let mut gone = listed("gone", State::Revoked);
    gone.revoked = Some(lemonfiber_core::instant::Instant::from_seconds(
        1_759_651_200,
    ));
    Listing {
        keys: vec![listed("assistant", State::Active), gone],
        revoked: Some("gone".to_owned()),
        purposes: "What a purpose is worth.".to_owned(),
        rehearsed: false,
    }
}
