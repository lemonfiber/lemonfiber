use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use lemonfiber_manifest::container::Network;
use serde_norway::Value as Yaml;

use super::super::{generated, Project, Reading, UNFENCED};
use super::{carried, entry, environment, ports, sequence, text};
use crate::fence::Unfenced;
use crate::plugin::Installed;
use crate::test_support::{a_placed, an_installed};
use crate::wiring::Chosen;

const FENCE: &str = "ghcr.io/lemonfiber/fence:0.18.0@sha256:9e1d000000000000000000000000000000000000000000000000000000000000";

const HOST: [IpAddr; 2] = [
    IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)),
    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
];

fn fenced(id: &str) -> Installed {
    let mut solver = a_placed(id, &[], Some(18191));
    solver.fields.network = Some(Network {
        via: None,
        fenced: true,
    });
    an_installed(id, vec![solver])
}

fn written(installed: &[Installed], fence: Option<&str>, host: &[IpAddr]) -> Project {
    generated(
        &carried(),
        installed,
        &Chosen::read(None),
        &Reading {
            fence,
            host,
            ..Reading::bare()
        },
    )
}

fn waits_on(entry: &Yaml, host: &str) -> (String, bool) {
    let waiting = entry
        .get("depends_on")
        .and_then(|depends| depends.get(host))
        .cloned()
        .unwrap_or_default();
    (
        text(&waiting, "condition"),
        waiting
            .get("restart")
            .and_then(Yaml::as_bool)
            .unwrap_or_default(),
    )
}

#[test]
fn a_fenced_service_runs_inside_its_fence_which_holds_its_networks_and_ports_and_starts_first() {
    let project = written(&[fenced("solver")], Some(FENCE), &HOST);
    let solver = entry(&project, "solver");
    let fence = entry(&project, "solver-fence");

    assert_eq!(text(&solver, "network_mode"), "service:solver-fence");
    assert!(ports(&solver).is_empty());
    assert!(solver.get("networks").is_none());
    assert_eq!(
        waits_on(&solver, "solver-fence"),
        ("service_healthy".to_owned(), true)
    );
    assert_eq!(text(&fence, "image"), FENCE);
    assert!(ports(&fence).iter().any(|port| port.contains("18191")));
    assert!(fence.get("networks").is_some());
    assert_eq!(
        environment(&fence, crate::fence::HOST_ADDRESSES),
        "203.0.113.7,2001:db8::1"
    );
    assert_eq!(sequence(&fence, "profiles"), sequence(&solver, "profiles"));
    assert!(project.unfenced.is_empty());
}

/// The reason the fence may run as root: every capability dropped but the one it needs,
/// no privilege gained after it starts, and nothing it can write.
#[test]
fn the_fence_holds_net_admin_alone_gains_no_privilege_and_writes_nothing() {
    let project = written(&[fenced("solver")], Some(FENCE), &HOST);
    let fence = entry(&project, "solver-fence");

    assert_eq!(sequence(&fence, "cap_drop"), ["ALL"]);
    assert_eq!(sequence(&fence, "cap_add"), ["NET_ADMIN"]);
    assert_eq!(sequence(&fence, "security_opt"), ["no-new-privileges:true"]);
    assert_eq!(fence.get("read_only").and_then(Yaml::as_bool), Some(true));
    assert!(sequence(&fence, "volumes").is_empty());
    assert!(!sequence(&entry(&project, "solver"), "cap_add").contains(&"NET_ADMIN".to_owned()));
}

#[test]
fn a_fenced_service_no_fence_can_be_written_for_is_held_back_on_no_network() {
    let mut taken = fenced("solver");
    taken.services.push(a_placed("solver-fence", &[], None));
    for (installed, fence, host, why) in [
        (fenced("solver"), None, HOST.as_slice(), Unfenced::Unpinned),
        (
            fenced("solver"),
            Some(FENCE),
            [].as_slice(),
            Unfenced::Unaddressed,
        ),
        (taken, Some(FENCE), HOST.as_slice(), Unfenced::Taken),
    ] {
        let project = written(&[installed], fence, host);
        let solver = entry(&project, "solver");

        assert_eq!(sequence(&solver, "profiles"), [UNFENCED], "{why:?}");
        assert_eq!(text(&solver, "network_mode"), "none", "{why:?}");
        assert!(ports(&solver).is_empty(), "{why:?}");
        assert_eq!(project.unfenced, vec![("solver".to_owned(), why)]);
        assert_ne!(
            text(&entry(&project, "solver-fence"), "image"),
            FENCE,
            "{why:?}"
        );
    }
}
