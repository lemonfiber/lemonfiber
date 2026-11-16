//! An egress service, held to holding nothing.

use super::*;

/// A service carrying a sibling out of the machine holds nothing: confined, with no secret,
/// port, data mount or capability address, and no other network beside it; and it never
/// carries itself.
#[test]
fn an_egress_service_holds_nothing_and_never_carries_itself() {
    let constant: BTreeMap<String, Value> = toml::from_str(r#"A = "x""#).unwrap_or_default();
    let asking: BTreeMap<String, Value> =
        toml::from_str(r#"E = { url_of = "indexer.search" }"#).unwrap_or_default();
    let egress = Network {
        via: None,
        fenced: true,
        internal: false,
        egress_for: Some("plex".to_owned()),
    };
    let carried = |container: &Container<'_>| -> Vec<String> {
        wrong(container)
            .into_iter()
            .filter(|said| said.contains("network.egress_for: "))
            .collect()
    };
    let mut holding = plain(&asking);
    holding.network = Some(&egress);
    let held = carried(&holding);
    for what in [
        "is fenced beside it",
        "is not confined",
        "declares a secret",
        "takes the data mount",
        "publishes a port",
        "asks the address of a capability",
    ] {
        assert!(
            held.iter().any(|said| said.contains(what)),
            "{what}: {held:?}"
        );
    }

    let bare = Network {
        fenced: false,
        ..egress.clone()
    };
    let mut empty = plain(&constant);
    empty.network = Some(&bare);
    empty.posture = Posture::Confined;
    empty.secrets = &[];
    empty.port = None;
    empty.takes_data = false;
    assert!(carried(&empty).is_empty(), "{:?}", carried(&empty));
    let itself = Network {
        internal: false,
        egress_for: Some(empty.id.to_owned()),
        ..bare.clone()
    };
    empty.network = Some(&itself);
    assert!(carried(&empty)
        .iter()
        .any(|said| said.contains("names the service itself")));
}
