//! The stack the bundle assembles, held to the media stack it replaces: every service
//! the same in every field, but for the differences named here until each is levelled,
//! and for the secrets a part declares and the variables naming them, which the media
//! stack's own services do not declare.

use std::collections::BTreeSet;
use std::path::Path;

use toml::{Table, Value};

const CATALOGUE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/plugins");
const MEDIA_STACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/media-stack");

/// Services the media stack runs that the bundle installs rather than assembles.
const NOT_ASSEMBLED: [&str; 7] = [
    "audiobookshelf",
    "bazarr",
    "bindery",
    "jellyfin",
    "nzbhydra2",
    "qbittorrent",
    "sabnzbd",
];

/// Fields of a service that differ between the two, by service and field: the guarded
/// door and the proxy run the images lemonfiber builds for them, a part released here
/// is tagged by its release without the `v`, and the gate is pinned at a commit on its
/// main branch past the release the media stack holds. Each part filling a capability
/// the media stack's own service does not declare provides and claims it in its plugin;
/// the proxy runs as its own user; the gate says the port it listens on; and the door
/// says it asks the pass service about a live stream.
const DIFFERING: [(&str, &str); 24] = [
    ("caddy", "claim"),
    ("caddy", "digest"),
    ("caddy", "image"),
    ("caddy", "provides"),
    ("caddy", "tag"),
    ("caddy", "user_mode"),
    ("decline", "claim"),
    ("decline", "provides"),
    ("decline", "tag"),
    ("door", "asks_for"),
    ("door", "claim"),
    ("door", "digest"),
    ("door", "image"),
    ("door", "provides"),
    ("door", "tag"),
    ("recyclarr", "claim"),
    ("recyclarr", "provides"),
    ("request-gate", "claim"),
    ("request-gate", "digest"),
    ("request-gate", "listens"),
    ("request-gate", "provides"),
    ("request-gate", "tag"),
    ("unpackerr", "claim"),
    ("unpackerr", "provides"),
];

/// The asks the embedded stack makes that the media stack does not, by asker and
/// capability: each service given a household hostname asks the proxy, each whose
/// dashboard panel reads it asks the dashboard, the request service asks the gate, and
/// the gate, quality sync, decline and the guarded door ask what they act on.
const ASKED_HERE: [(&str, &str); 16] = [
    ("calibre-web-automated", "proxy.front"),
    ("decline", "identity.source"),
    ("door", "media.serve"),
    ("door", "proxy.front"),
    ("homepage", "proxy.front"),
    ("lidarr", "dashboard.show"),
    ("navidrome", "dashboard.show"),
    ("prowlarr", "dashboard.show"),
    ("radarr", "dashboard.show"),
    ("recyclarr", "library.curate"),
    ("request-gate", "identity.source"),
    ("request-gate", "library.curate"),
    ("seerr", "dashboard.show"),
    ("seerr", "proxy.front"),
    ("seerr", "request.gate"),
    ("sonarr", "dashboard.show"),
];

/// The links by name the media stack makes that the embedded stack does not, by asker
/// and the service named: the guarded door asks for media instead.
const NAMED_THERE: [(&str, &str); 1] = [("door", "jellyfin")];

/// Each variable a part takes from a secret it declares, as its service, the variable
/// and the secret: the media stack's own services declare none of them.
const FROM_SECRETS: [(&str, &str, &str); 6] = [
    ("gluetun", "WIREGUARD_PRIVATE_KEY", "wireguard-key"),
    ("recyclarr", "RADARR_API_KEY", "movies-key"),
    ("recyclarr", "SONARR_API_KEY", "tv-key"),
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

fn without_secrets(service: &Table) -> Table {
    let mut kept = service.clone();
    kept.remove("secret");
    if let Some(env) = kept.get_mut("env").and_then(Value::as_table_mut) {
        env.retain(|_, value| secret_named(value).is_none());
    }
    kept
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

fn assembled() -> String {
    let root = Path::new(CATALOGUE);
    let text = std::fs::read_to_string(root.join(lemonfiber_bundle::FILE)).unwrap_or_default();
    let Ok((_, held)) = lemonfiber_bundle::read(root) else {
        unreachable!("the embedded bundle reads; the build refuses one that does not");
    };
    lemonfiber_bundle::assembled(&text, &held, env!("CARGO_PKG_VERSION")).unwrap_or_default()
}

fn media_stack() -> String {
    lemonfiber_manifest::read(Path::new(MEDIA_STACK)).unwrap_or_default()
}

#[test]
fn the_assembled_stack_reads_as_a_stack_manifest() {
    assert!(lemonfiber_manifest::Manifest::from_toml(&assembled()).is_ok());
}

#[test]
fn every_service_the_media_stack_runs_is_assembled_but_those_named() {
    let ours: BTreeSet<String> = services(&assembled())
        .iter()
        .map(|one| id(one).to_owned())
        .collect();
    let missing: BTreeSet<String> = services(&media_stack())
        .iter()
        .map(|one| id(one).to_owned())
        .filter(|id| !ours.contains(id))
        .collect();
    let named: BTreeSet<String> = NOT_ASSEMBLED.iter().map(|id| (*id).to_owned()).collect();
    assert_eq!(missing, named);
}

#[test]
fn every_assembled_service_is_the_media_stacks_but_where_named() {
    let ours: Vec<Table> = services(&assembled()).iter().map(without_secrets).collect();
    let mut differing = BTreeSet::new();
    for theirs in services(&media_stack()) {
        let Some(mine) = ours.iter().find(|one| id(one) == id(&theirs)) else {
            continue;
        };
        for key in theirs.keys().chain(mine.keys()) {
            if theirs.get(key) != mine.get(key) {
                differing.insert((id(&theirs).to_owned(), key.clone()));
            }
        }
    }
    let named: BTreeSet<(String, String)> = DIFFERING
        .iter()
        .map(|(id, key)| ((*id).to_owned(), (*key).to_owned()))
        .collect();
    assert_eq!(differing, named);
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

fn table_rows(text: &str, name: &str) -> BTreeSet<String> {
    let parsed: Table = toml::from_str(text).unwrap_or_default();
    parsed
        .get(name)
        .and_then(Value::as_array)
        .map(|rows| rows.iter().map(ToString::to_string).collect())
        .unwrap_or_default()
}

#[test]
fn the_embedded_stack_runs_every_service_the_media_stack_runs() {
    let ours: BTreeSet<String> = services(lemonfiber_bundled::STACK)
        .iter()
        .map(|one| id(one).to_owned())
        .collect();
    let theirs: BTreeSet<String> = services(&media_stack())
        .iter()
        .map(|one| id(one).to_owned())
        .collect();
    assert_eq!(ours, theirs);
}

#[test]
fn an_installed_part_is_embedded_as_the_media_stack_declares_it() {
    for installed in NOT_ASSEMBLED {
        let find = |text: &str| services(text).into_iter().find(|one| id(one) == installed);
        assert_eq!(
            find(lemonfiber_bundled::STACK),
            find(&media_stack()),
            "{installed}"
        );
    }
}

#[test]
fn the_embedded_stacks_own_tables_are_the_media_stacks() {
    for name in ["profile", "form", "removed"] {
        assert_eq!(
            table_rows(lemonfiber_bundled::STACK, name),
            table_rows(&media_stack(), name),
            "{name}"
        );
    }
}

/// A wiring row as the end it reaches: what it asks for, or the service it names.
fn ends(text: &str) -> BTreeSet<(String, String)> {
    let parsed: Table = toml::from_str(text).unwrap_or_default();
    parsed
        .get("wiring")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let by = row.get("by")?.as_str()?;
            let end = row.get("asks").or_else(|| row.get("to"))?.as_str()?;
            Some((by.to_owned(), end.to_owned()))
        })
        .collect()
}

#[test]
fn the_embedded_stacks_wiring_is_the_media_stacks_but_where_named() {
    let ours = ends(lemonfiber_bundled::STACK);
    let theirs = ends(&media_stack());
    let named = |rows: &[(&str, &str)]| -> BTreeSet<(String, String)> {
        rows.iter()
            .map(|(by, end)| ((*by).to_owned(), (*end).to_owned()))
            .collect()
    };
    assert_eq!(
        ours.difference(&theirs).cloned().collect::<BTreeSet<_>>(),
        named(&ASKED_HERE)
    );
    assert_eq!(
        theirs.difference(&ours).cloned().collect::<BTreeSet<_>>(),
        named(&NAMED_THERE)
    );
}
