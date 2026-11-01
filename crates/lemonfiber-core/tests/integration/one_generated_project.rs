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

/// The carried stack, generated with every part the bundle installs installed.
fn installed() -> Project {
    let Ok(manifest) = Manifest::from_toml(STACK) else {
        unreachable!("the embedded stack parses; the manifest crate asserts it separately");
    };
    let parts = lemonfiber_testing::adapted::installed_parts();
    generated(&manifest, &parts, &Chosen::read(None), &Reading::bare())
}

/// The carried stack, generated with its guards moved onto their plugins, as every
/// machine running this release holds it.
fn guarded() -> Project {
    let Ok(manifest) = Manifest::from_toml(STACK) else {
        unreachable!("the embedded stack parses; the manifest crate asserts it separately");
    };
    let guards = lemonfiber_testing::guards::both_installed();
    generated(&manifest, &guards, &Chosen::read(None), &Reading::bare())
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

/// The golden files the project is held to, each with the part of it it holds: its name
/// and services, then its networks and volumes, apart so that neither file grows past
/// what one is allowed.
fn golden(project: &Project) -> [(PathBuf, String); 2] {
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let (services, networks) = project.text.split_at(
        project
            .text
            .find("\nnetworks:")
            .map_or(project.text.len(), |at| at + 1),
    );
    [
        (at.join("project.yml"), services.to_owned()),
        (at.join("project.networks.yml"), networks.to_owned()),
    ]
}

#[test]
fn the_carried_stack_generates_its_golden_project() {
    for (path, text) in golden(&shipped()) {
        if std::env::var_os("LEMONFIBER_BLESS_GOLDEN").is_some() {
            let _ = fs::write(&path, &text);
            continue;
        }
        let recorded = fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            recorded,
            text,
            "the generated project moved; if intended, bless {} and read the diff",
            path.display()
        );
    }
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

/// The networks a default Docker install draws from its own pools: sixteen `/16`s of
/// `172.16.0.0/12` and sixteen `/20`s of `192.168.0.0/16`, less the one its own bridge
/// holds.
const DEFAULT_POOLS: usize = 31;

/// Every network the document declares, with the subnet it names where it names one.
fn declared_subnets(document: &Value) -> Vec<(String, Option<(Ipv4Addr, u8)>)> {
    document
        .get("networks")
        .and_then(Value::as_mapping)
        .into_iter()
        .flatten()
        .map(|(name, declared)| {
            let subnet = declared
                .get("ipam")
                .and_then(|ipam| ipam.get("config")?.get(0)?.get("subnet")?.as_str())
                .and_then(|subnet| {
                    let (first, prefix) = subnet.split_once('/')?;
                    Some((first.parse().ok()?, prefix.parse().ok()?))
                });
            (name.as_str().unwrap_or_default().to_owned(), subnet)
        })
        .collect()
}

#[test]
fn the_engine_draws_no_network_from_its_own_pools_and_no_two_subnets_meet() {
    let declared = declared_subnets(&document(&guarded()));
    let drawn: Vec<&String> = declared
        .iter()
        .filter(|(_, subnet)| subnet.is_none())
        .map(|(name, _)| name)
        .collect();

    assert!(
        declared.len() > DEFAULT_POOLS,
        "the stack asks for more networks than a default install's pools hold, which is why each names its subnet"
    );
    assert!(drawn.is_empty(), "drawn from the engine's pools: {drawn:?}");
    let subnets: Vec<(Ipv4Addr, u8)> = declared.iter().filter_map(|(_, subnet)| *subnet).collect();
    for (at, subnet) in subnets.iter().enumerate() {
        assert!(
            within(subnet.0, lemonfiber_core::topology::RANGE),
            "{subnet:?} outside the range lemonfiber holds"
        );
        for other in subnets.iter().skip(at + 1) {
            assert!(
                !within(other.0, *subnet) && !within(subnet.0, *other),
                "{subnet:?} meets {other:?}"
            );
        }
    }
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

/// The fixed addresses `service` holds in the shipped project, in any order.
fn fixed(project: &Project, service: &str) -> BTreeSet<Ipv4Addr> {
    match project.topology.placed.get(service) {
        Some(Placed::On(on)) => on.values().flatten().copied().collect(),
        _ => BTreeSet::new(),
    }
}

#[test]
fn the_door_and_the_proxy_hold_the_addresses_each_side_trusts() {
    let project = installed();
    assert_eq!(
        fixed(&project, "door"),
        BTreeSet::from([
            lemonfiber_core::topology::DOOR_ON_PROXY,
            lemonfiber_core::topology::DOOR_ON_MEDIA
        ])
    );
    assert_eq!(
        fixed(&project, "caddy"),
        BTreeSet::from([lemonfiber_core::topology::PROXY_ON_PROXY])
    );
}
