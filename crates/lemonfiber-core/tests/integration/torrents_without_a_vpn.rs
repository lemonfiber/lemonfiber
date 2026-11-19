//! A stack whose operator chose torrents without a VPN: the torrent client on its own
//! network, and no egress guard planned or started for it.

use lemonfiber_contract::capabilities::network::egress;
use lemonfiber_core::config::{Protocols, Supported};
use lemonfiber_core::plugin::Installed;
use lemonfiber_core::project::{generated, Project, Reading, REPLACED};
use lemonfiber_core::stack::closure::{everything, resolve};
use lemonfiber_core::topology::Placed;
use lemonfiber_core::wiring::Chosen;
use lemonfiber_manifest::Manifest;
use serde_norway::Value;

/// The stack this repository carries.
fn carried() -> Manifest {
    Manifest::from_toml(lemonfiber_bundled::STACK).unwrap_or_else(|_| {
        unreachable!("the embedded stack parses; the manifest crate asserts it separately")
    })
}

/// Torrents configured, and chosen without a VPN.
const UNGUARDED: Supported = Supported {
    protocols: Protocols::both(),
    vpn_declined: true,
};

/// The carried stack's egress guard and the torrent client it runs, by id.
fn guard_and_client(manifest: &Manifest) -> (String, String) {
    let guard = manifest
        .services
        .iter()
        .find(|service| service.guards_egress())
        .map(|service| service.id.clone())
        .unwrap_or_default();
    let client = manifest
        .services
        .iter()
        .find(|service| {
            service.network.as_ref().and_then(|network| network.via()) == Some(egress::CAPABILITY)
        })
        .map(|service| service.id.clone())
        .unwrap_or_default();
    (guard, client)
}

/// The project the carried stack and `installed` come to with the egress guard declined.
fn declined(manifest: &Manifest, installed: &[Installed]) -> Project {
    let chosen = Chosen::read(None).declining(egress::CAPABILITY);
    generated(manifest, installed, &chosen, &Reading::bare())
}

/// The entry the project writes for `id`.
fn entry(project: &Project, id: &str) -> Value {
    let document: Value = serde_norway::from_str(&project.text).unwrap_or_default();
    document
        .get("services")
        .and_then(|services| services.get(id))
        .cloned()
        .unwrap_or_default()
}

/// The profiles an entry is written under.
fn profiles(entry: &Value) -> Vec<String> {
    entry
        .get("profiles")
        .and_then(Value::as_sequence)
        .map(|names| {
            names
                .iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_torrent_client_runs_on_its_own_network_and_the_guard_is_never_started() {
    let manifest = carried();
    let (guard, client) = guard_and_client(&manifest);
    assert!(!guard.is_empty() && !client.is_empty());
    let parts = lemonfiber_testing::adapted::installed_parts();
    for project in [declined(&manifest, &[]), declined(&manifest, &parts)] {
        let written = entry(&project, &client);
        assert!(written.get("network_mode").is_none(), "{written:?}");
        assert!(written.get("depends_on").is_none(), "{written:?}");
        assert!(written.get("networks").is_some(), "{written:?}");
        assert!(
            matches!(project.topology.placed.get(&client), Some(Placed::On(on)) if !on.is_empty())
        );
        assert_eq!(profiles(&entry(&project, &guard)), [REPLACED]);
        assert!(!project
            .topology
            .placed
            .values()
            .any(|placed| *placed == Placed::Inside(guard.clone())));
    }
}

#[test]
fn every_service_of_the_installed_guard_is_left_out_of_what_starts() {
    let manifest = carried();
    let parts = lemonfiber_testing::adapted::installed_parts();
    let guarding: Vec<String> = parts
        .iter()
        .filter(|one| {
            one.services
                .iter()
                .any(|placed| placed.provides.iter().any(|one| one == egress::CAPABILITY))
        })
        .flat_map(|one| one.services.iter().map(|placed| placed.service.clone()))
        .collect();
    assert!(!guarding.is_empty());
    let project = declined(&manifest, &parts);
    for service in &guarding {
        assert_eq!(profiles(&entry(&project, service)), [REPLACED], "{service}");
    }
}

#[test]
fn the_plan_holds_the_torrent_client_and_no_egress_guard() {
    let manifest = carried();
    let (guard, client) = guard_and_client(&manifest);
    let named = manifest
        .forms
        .iter()
        .map(|form| form.id.clone())
        .find(|form| {
            resolve(&manifest, std::slice::from_ref(form), Protocols::both())
                .is_ok_and(|plan| plan.services.contains(&guard))
        })
        .unwrap_or_default();
    let plans = [
        resolve(&manifest, std::slice::from_ref(&named), UNGUARDED),
        everything(&manifest, UNGUARDED),
    ];
    for plan in plans {
        let services = plan.map(|plan| plan.services).unwrap_or_default();
        assert!(services.contains(&client), "{services:?}");
        assert!(!services.contains(&guard), "{services:?}");
    }
    let guarded = everything(&manifest, Protocols::both()).map(|plan| plan.services);
    assert!(guarded.is_ok_and(|services| services.contains(&guard)));
}
