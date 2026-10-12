//! The one project lemonfiber generates from the stack it carries.
//!
//! Regenerate the golden file after an intended change:
//!
//! ```sh
//! LEMONFIBER_BLESS_GOLDEN=1 cargo test -p lemonfiber-core --test integration one_generated_project
//! ```
//!
//! Then read the diff.

use std::collections::BTreeSet;
use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use lemonfiber_core::project::{generated, Project, Reading};
use lemonfiber_core::topology::Placed;
use lemonfiber_core::wiring::Chosen;
use lemonfiber_manifest::Manifest;
use serde_norway::Value;

/// The stack this repository carries.
const STACK: &str = lemonfiber_bundled::STACK;

/// The carried stack, generated with nothing installed and nothing recorded.
fn shipped() -> Project {
    let Ok(manifest) = Manifest::from_toml(STACK) else {
        unreachable!("the embedded stack parses; the manifest crate asserts it separately");
    };
    generated(&manifest, &[], &Chosen::read(None), &Reading::bare())
}

/// The project's document, read back.
fn document(project: &Project) -> Value {
    serde_norway::from_str(&project.text).unwrap_or_default()
}

/// Every service entry of a document, by name.
fn services(document: &Value) -> Vec<(String, Value)> {
    document
        .get("services")
        .and_then(Value::as_mapping)
        .map(|mapping| {
            mapping
                .iter()
                .filter_map(|(name, entry)| Some((name.as_str()?.to_owned(), entry.clone())))
                .collect()
        })
        .unwrap_or_default()
}

/// The network names an entry joins.
fn joined(entry: &Value) -> Vec<String> {
    match entry.get("networks") {
        Some(Value::Sequence(names)) => names
            .iter()
            .filter_map(|name| name.as_str().map(str::to_owned))
            .collect(),
        Some(Value::Mapping(names)) => names
            .keys()
            .filter_map(|name| name.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/project.yml")
}

#[test]
fn the_carried_stack_generates_its_golden_project() {
    let project = shipped();
    let path = golden();
    if std::env::var_os("LEMONFIBER_BLESS_GOLDEN").is_some() {
        let _ = fs::write(&path, &project.text);
        return;
    }
    let recorded = fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        recorded,
        project.text,
        "the generated project moved; if intended, bless {} and read the diff",
        path.display()
    );
}

#[test]
fn generating_twice_writes_the_same_project() {
    assert_eq!(shipped(), shipped());
}

#[test]
fn no_entry_falls_back_on_the_implicit_default_network() {
    let document = document(&shipped());
    let services = services(&document);
    assert!(!services.is_empty(), "the carried stack writes services");
    for (name, entry) in services {
        assert!(
            entry.get("networks").is_some() || entry.get("network_mode").is_some(),
            "{name} names neither its networks nor a network mode, so Compose would put it on `default`"
        );
        assert!(
            !joined(&entry).iter().any(|network| network == "default"),
            "{name} joins `default`"
        );
    }
}

#[test]
fn every_network_an_entry_joins_is_declared() {
    let document = document(&shipped());
    let declared: BTreeSet<String> = document
        .get("networks")
        .and_then(Value::as_mapping)
        .map(|mapping| {
            mapping
                .keys()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    for (name, entry) in services(&document) {
        for network in joined(&entry) {
            assert!(
                declared.contains(&network),
                "{name} joins undeclared {network}"
            );
        }
    }
}

#[test]
fn every_fixed_address_lies_in_its_subnet_and_outside_what_the_engine_hands_out() {
    let project = shipped();
    for placed in project.topology.placed.values() {
        let Placed::On(on) = placed else { continue };
        for (network, address) in on {
            let Some(address) = address else { continue };
            let Some(fixed) = project
                .topology
                .networks
                .get(network)
                .and_then(|declared| declared.fixed)
            else {
                unreachable!("{network} fixes {address} without a fixed subnet");
            };
            assert!(within(*address, fixed.whole), "{address} outside {network}");
            assert!(
                !within(*address, fixed.handed_out),
                "{address} on {network} lies in what the engine hands out"
            );
        }
    }
}

/// Whether `address` lies in `block`.
fn within(address: Ipv4Addr, (first, prefix): (Ipv4Addr, u8)) -> bool {
    let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
    u32::from(address) & mask == u32::from(first) & mask
}

#[test]
fn no_service_mounts_the_library_twice() {
    for (name, entry) in services(&document(&shipped())) {
        let libraries = entry
            .get("volumes")
            .and_then(Value::as_sequence)
            .map(|volumes| {
                volumes
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|volume| volume.split(':').nth(1) == Some("/data"))
                    .count()
            })
            .unwrap_or_default();
        assert!(libraries <= 1, "{name} mounts /data {libraries} times");
    }
}

#[test]
fn a_service_the_host_runs_gets_no_entry() {
    let Ok(manifest) = Manifest::from_toml(STACK) else {
        unreachable!("the embedded stack parses");
    };
    let written: BTreeSet<String> = services(&document(&shipped()))
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    for service in &manifest.services {
        assert_eq!(
            written.contains(&service.id),
            !service.host_managed,
            "{} is host managed: {}",
            service.id,
            service.host_managed
        );
    }
}

/// The carried stack's static Compose files, which the generated project replaces at the
/// cutover. Read here only to hold the two to the same images and published ports while
/// both exist.
const STATIC: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/media-stack/compose"
);

/// Every service the static files declare, with its image and published ports.
fn static_services() -> Vec<(String, Option<String>, BTreeSet<String>)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(STATIC).into_iter().flatten().flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name.starts_with('_') || !name.ends_with(".yml") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap_or_default();
        let document: Value = serde_norway::from_str(&text).unwrap_or_default();
        for (service, entry) in services(&document) {
            found.push((
                service,
                entry
                    .get("image")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                published(&entry),
            ));
        }
    }
    found
}

/// The ports an entry publishes.
fn published(entry: &Value) -> BTreeSet<String> {
    entry
        .get("ports")
        .and_then(Value::as_sequence)
        .map(|ports| {
            ports
                .iter()
                .filter_map(|port| port.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_static_service_is_generated_with_its_image_and_ports() {
    let generated = services(&document(&shipped()));
    let statics = static_services();
    assert!(!statics.is_empty(), "the static files declare services");
    for (service, image, ports) in statics {
        let Some((_, entry)) = generated.iter().find(|(name, _)| *name == service) else {
            unreachable!("{service} is in the static files and not in the generated project");
        };
        assert_eq!(
            entry
                .get("image")
                .and_then(Value::as_str)
                .map(str::to_owned),
            image,
            "{service}'s image"
        );
        assert_eq!(published(entry), ports, "{service}'s published ports");
    }
}
