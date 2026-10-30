//! The stack the bundle assembles: a stack manifest lemonfiber reads, a service for every
//! part the bundle pins, each installed part's as its own copy declares the service it
//! takes the place of and providing what the part provides, and the secrets each part's
//! variables are filled from.

use std::collections::BTreeSet;
use std::path::Path;

use toml::{Table, Value};

const CATALOGUE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/plugins");

/// The fields of an installed part's upstream service its stack service does not carry.
const PLUGIN_ONLY: [&str; 5] = ["replaces", "native", "env", "speaks", "contract"];

/// Each variable a part takes from a secret it declares, as its service, the variable
/// and the secret: the media stack's own services declare none of them.
const FROM_SECRETS: [(&str, &str, &str); 7] = [
    ("gluetun", "WIREGUARD_PRIVATE_KEY", "wireguard-key"),
    (
        "gluetun",
        "WIREGUARD_PRESHARED_KEY",
        "wireguard-preshared-key",
    ),
    ("gluetun", "OPENVPN_USER", "openvpn-user"),
    ("gluetun", "OPENVPN_PASSWORD", "openvpn-password"),
    ("unpackerr", "UN_LIDARR_0_API_KEY", "music-key"),
    ("unpackerr", "UN_RADARR_0_API_KEY", "movies-key"),
    ("unpackerr", "UN_SONARR_0_API_KEY", "tv-key"),
];

fn services(text: &str) -> Vec<Table> {
    let parsed: Table = toml::from_str(text).unwrap_or_default();
    parsed
        .get("service")
        .and_then(Value::as_array)
        .map(|services| {
            services
                .iter()
                .filter_map(|one| one.as_table().cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn id(service: &Table) -> &str {
    service
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn secret_named(value: &Value) -> Option<&str> {
    value.as_table()?.get("secret")?.as_str()
}

fn from_secrets(service: &Table) -> Vec<(String, String, String)> {
    service
        .get("env")
        .and_then(Value::as_table)
        .into_iter()
        .flatten()
        .filter_map(|(variable, value)| {
            secret_named(value)
                .map(|secret| (id(service).to_owned(), variable.clone(), secret.to_owned()))
        })
        .collect()
}

fn declared_secrets(service: &Table) -> BTreeSet<String> {
    service
        .get("secret")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|secret| secret.get("id").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect()
}

fn held() -> Vec<lemonfiber_bundle::Held> {
    let Ok((_, held)) = lemonfiber_bundle::read(Path::new(CATALOGUE)) else {
        unreachable!("the embedded bundle reads; the build refuses one that does not");
    };
    held
}

fn assembled() -> String {
    let root = Path::new(CATALOGUE);
    let text = std::fs::read_to_string(root.join(lemonfiber_bundle::FILE)).unwrap_or_default();
    lemonfiber_bundle::assembled(&text, &held(), env!("CARGO_PKG_VERSION")).unwrap_or_default()
}

#[test]
fn the_assembled_stack_reads_as_a_stack_manifest() {
    assert!(lemonfiber_manifest::Manifest::from_toml(&assembled()).is_ok());
}

#[test]
fn the_embedded_stack_runs_a_service_for_every_part_the_bundle_pins() {
    let ours: BTreeSet<String> = services(lemonfiber_bundled::STACK)
        .iter()
        .map(|one| id(one).to_owned())
        .collect();
    let pinned: BTreeSet<String> = held().into_iter().map(|one| one.pin.id).collect();
    assert!(!pinned.is_empty());
    assert_eq!(ours, pinned);
}

#[test]
fn an_installed_part_is_embedded_as_its_copy_declares_the_service_it_takes_the_place_of() {
    let installed: Vec<lemonfiber_bundle::Held> = held()
        .into_iter()
        .filter(|one| one.pin.stage == Some(lemonfiber_bundle::Stage::Installed))
        .collect();
    assert!(!installed.is_empty());
    for one in installed {
        let embedded = services(lemonfiber_bundled::STACK)
            .into_iter()
            .find(|service| id(service) == one.pin.id)
            .unwrap_or_default();
        let declared = services(&one.copy)
            .into_iter()
            .find(|service| {
                service.get("replaces").and_then(Value::as_str) == Some(one.pin.id.as_str())
            })
            .unwrap_or_default();
        assert!(!declared.is_empty(), "{}", one.pin.id);
        for (field, value) in &declared {
            if PLUGIN_ONLY.contains(&field.as_str()) {
                continue;
            }
            assert_eq!(embedded.get(field), Some(value), "{} {field}", one.pin.id);
        }
        for field in ["replaces", "native", "api", "claim", "speaks", "contract"] {
            assert!(embedded.get(field).is_none(), "{} {field}", one.pin.id);
        }
        let copied = services(&one.copy);
        let provided: BTreeSet<&str> = copied
            .iter()
            .filter_map(|service| service.get("provides").and_then(Value::as_array))
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let embedded_provides: BTreeSet<&str> = embedded
            .get("provides")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(embedded_provides, provided, "{}", one.pin.id);
    }
}

#[test]
fn every_variable_a_part_takes_from_a_secret_is_embedded_as_named() {
    let found: BTreeSet<(String, String, String)> = services(lemonfiber_bundled::STACK)
        .iter()
        .flat_map(from_secrets)
        .collect();
    let named: BTreeSet<(String, String, String)> = FROM_SECRETS
        .iter()
        .map(|(service, variable, secret)| {
            (
                (*service).to_owned(),
                (*variable).to_owned(),
                (*secret).to_owned(),
            )
        })
        .collect();
    assert_eq!(found, named);
}

#[test]
fn every_embedded_service_declares_exactly_the_secrets_its_env_names() {
    for service in services(lemonfiber_bundled::STACK) {
        let named: BTreeSet<String> = from_secrets(&service)
            .into_iter()
            .map(|(_, _, secret)| secret)
            .collect();
        assert_eq!(declared_secrets(&service), named, "{}", id(&service));
    }
}

#[test]
fn the_embedded_gluetun_declares_its_wireguard_key_as_its_own() {
    let gluetun = services(lemonfiber_bundled::STACK)
        .into_iter()
        .find(|one| id(one) == "gluetun")
        .unwrap_or_default();
    let declared = gluetun
        .get("secret")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|secret| secret.get("id").and_then(Value::as_str) == Some("wireguard-key"));
    assert_eq!(
        declared
            .and_then(|secret| secret.get("of"))
            .and_then(Value::as_str),
        Some("gluetun")
    );
    assert!(declared
        .and_then(|secret| secret.get("why"))
        .and_then(Value::as_str)
        .is_some_and(|why| !why.is_empty()));
}
