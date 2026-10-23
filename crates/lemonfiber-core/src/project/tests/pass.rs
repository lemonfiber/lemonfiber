use serde_norway::Value as Yaml;

use super::super::{generated, Reading};
use super::{carried, entry, ports, project, sequence, text};
use crate::test_support::{a_placed, an_installed};
use crate::wiring::Chosen;

const PINNED_PASS: &str = "ghcr.io/lemonfiber/lemonfiber:0.18.0@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

fn pinned() -> Reading<'static> {
    Reading {
        pass: Some(PINNED_PASS),
        ..Reading::bare()
    }
}

#[test]
fn the_pass_service_runs_its_pinned_image_alone_beside_the_guard() {
    let project = generated(&carried(), &[], &Chosen::read(None), &pinned());
    let pass = entry(&project, crate::pass::SERVICE);

    assert_eq!(text(&pass, "image"), PINNED_PASS);
    assert_eq!(
        sequence(&pass, "command"),
        ["pass", "--listen", "10.80.96.34:8090"]
    );
    assert_eq!(
        sequence(&pass, "profiles"),
        sequence(&entry(&project, "door"), "profiles")
    );
    assert_eq!(sequence(&pass, "volumes"), ["./config/pass:/config:ro"]);
    assert_eq!(sequence(&pass, "cap_drop"), ["ALL"]);
    assert_eq!(pass.get("read_only").and_then(Yaml::as_bool), Some(true));
    assert_eq!(text(&pass, "mem_limit"), "64m");
    assert!(ports(&pass).is_empty());
    let networks: Vec<String> = pass
        .get("networks")
        .and_then(Yaml::as_mapping)
        .map(|networks| {
            networks
                .keys()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(networks, ["pass__door"]);
    assert!(entry(&project, "door")
        .get("networks")
        .and_then(|networks| networks.get("pass__door"))
        .is_some());
    assert_eq!(
        project.topology.pass(),
        Some(std::net::SocketAddr::from((
            crate::topology::PASS_ON_PASS,
            crate::pass::PORT
        )))
    );
}

#[test]
fn no_pin_or_no_guard_runs_no_pass_service() {
    let unpinned = project(&carried(), &[]);
    let mut unguarded = carried();
    for service in &mut unguarded.services {
        service.provides.retain(|named| named != "stream.guard");
    }
    let unguarded = generated(&unguarded, &[], &Chosen::read(None), &pinned());

    for project in [unpinned, unguarded] {
        assert!(entry(&project, crate::pass::SERVICE).is_null());
        assert_eq!(project.topology.pass(), None);
        assert!(!project
            .topology
            .networks
            .keys()
            .any(|name| name.starts_with("pass__")));
    }
}

#[test]
fn a_service_holding_the_pass_services_id_leaves_the_pass_service_out() {
    let mut taken = a_placed(crate::pass::SERVICE, &[], None, None);
    taken.image = "example.invalid/impostor".to_owned();
    let project = generated(
        &carried(),
        &[an_installed("impostor", vec![taken])],
        &Chosen::read(None),
        &pinned(),
    );

    assert_eq!(project.topology.pass(), None);
    assert!(!project.topology.networks.contains_key("pass__door"));
    assert!(!text(&entry(&project, crate::pass::SERVICE), "image").contains("lemonfiber"));
    assert_eq!(project.text.matches("\n  pass:\n").count(), 1);
}
