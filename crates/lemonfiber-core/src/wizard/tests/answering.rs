//! The VPN answer as it is read, written and shown.

use std::collections::BTreeMap;

use super::super::{Guarded, Vpn};

/// What the guard is given to dial its tunnel, assembled so no credential sits here.
fn dialling() -> String {
    ["dialled", "not", "real"].join("-")
}

#[test]
fn a_vpn_answer_reads_whole_or_as_either_word_and_writes_whole() {
    let given = Guarded {
        settings: BTreeMap::from([("guard:vpn-type".to_owned(), "wireguard".to_owned())]),
        secrets: BTreeMap::from([("guard:wireguard-key".to_owned(), dialling())]),
    };
    let written = serde_json::to_string(&Vpn::Carrying(given.clone())).unwrap_or_default();

    assert_eq!(
        serde_json::from_str::<Vpn>(&written).ok(),
        Some(Vpn::Carrying(given))
    );
    assert_eq!(
        serde_json::from_str::<Vpn>("\"carrying\"").ok(),
        Some(Vpn::Carrying(Guarded::default()))
    );
    assert_eq!(
        serde_json::from_str::<Vpn>("\"absent\"").ok(),
        Some(Vpn::Absent)
    );
    assert_eq!(
        serde_json::to_string(&Vpn::Absent).ok().as_deref(),
        Some("\"absent\"")
    );
    assert!(serde_json::from_str::<Vpn>("{\"carrying\":{\"keys\":{}}}").is_err());
}

#[test]
fn a_vpn_answer_shown_for_debugging_names_its_secrets_and_never_their_values() {
    let dialled = dialling();
    let given = Guarded {
        settings: BTreeMap::from([("guard:vpn-type".to_owned(), "wireguard".to_owned())]),
        secrets: BTreeMap::from([("guard:wireguard-key".to_owned(), dialled.clone())]),
    };

    let shown = format!("{:?}", Vpn::Carrying(given));

    assert!(shown.contains("guard:wireguard-key") && shown.contains("wireguard"));
    assert!(!shown.contains(&dialled));
}
