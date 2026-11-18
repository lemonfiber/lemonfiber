//! A service held inside the stack: run inside a fence holding only what is inside its own
//! networks, handed each one's subnet and gateway, resolving names as written, and held
//! back where any of them was not answered.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};

use lemonfiber_manifest::container::Network;

use super::super::{generated, Project, Reading, UNFENCED};
use super::{carried, entry, environment, sequence, text};
use crate::fence::{Answered, Unfenced};
use crate::plugin::Installed;
use crate::test_support::{a_placed, an_installed};
use crate::topology::Placed;
use crate::wiring::Chosen;

const FENCE: &str = "ghcr.io/lemonfiber/fence:0.19.0@sha256:9e1d000000000000000000000000000000000000000000000000000000000000";

const HOST: [IpAddr; 1] = [IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))];

fn held_inside(id: &str) -> Installed {
    let mut dashboard = a_placed(id, &[], Some(4000));
    dashboard.fields.network = Some(Network {
        internal: true,
        ..Network::default()
    });
    an_installed(id, vec![dashboard])
}

fn written(installed: &[Installed], networks: &BTreeMap<String, Answered>) -> Project {
    generated(
        &carried(),
        installed,
        &Chosen::read(None),
        &Reading {
            fence: Some(FENCE),
            host: &HOST,
            nftables: true,
            networks,
            ..Reading::bare()
        },
    )
}

/// Each network the fence `fence` joins in `project`, answered by the engine with a
/// subnet of its own.
fn answered(project: &Project, fence: &str) -> BTreeMap<String, Answered> {
    let Some(Placed::On(on)) = project.topology.placed.get(fence) else {
        return BTreeMap::new();
    };
    on.keys()
        .zip(0_u8..)
        .filter_map(|(network, slot)| {
            let first = slot.checked_mul(8)?;
            let answered = Answered::of(
                &format!("10.80.9.{first}/29"),
                &format!("10.80.9.{}", first.checked_add(1)?),
                &format!("id-{network}"),
            )?;
            Some((network.clone(), answered))
        })
        .collect()
}

#[test]
fn a_service_held_inside_the_stack_runs_in_a_fence_handed_its_networks_resolving_names_as_written()
{
    let installed = [held_inside("dashy")];
    let networks = answered(&written(&installed, &BTreeMap::new()), "dashy-fence");
    assert!(!networks.is_empty());
    let project = written(&installed, &networks);
    let dashy = entry(&project, "dashy");
    let fence = entry(&project, "dashy-fence");

    assert!(project.unfenced.is_empty());
    assert_eq!(text(&dashy, "network_mode"), "service:dashy-fence");
    assert_eq!(sequence(&fence, "command"), ["stack"]);
    assert_eq!(sequence(&fence, "dns_search"), ["."]);
    assert_eq!(
        environment(&fence, crate::fence::NETWORKS),
        crate::fence::networks(networks.values())
    );
    assert_eq!(
        environment(&fence, crate::fence::HOST_ADDRESSES),
        "203.0.113.7"
    );
}

#[test]
fn a_service_held_inside_the_stack_whose_networks_were_not_answered_is_held_back() {
    let project = written(&[held_inside("dashy")], &BTreeMap::new());

    assert_eq!(project.unfenced, [("dashy".to_owned(), Unfenced::Unread)]);
    let dashy = entry(&project, "dashy");
    assert_eq!(sequence(&dashy, "profiles"), [UNFENCED]);
    assert_eq!(text(&dashy, "network_mode"), "none");
    assert!(entry(&project, "dashy-fence").is_null());
}

#[test]
fn a_fenced_service_keeps_its_outward_fence_with_no_networks_handed() {
    let mut solver = a_placed("solver", &[], Some(18191));
    solver.fields.network = Some(Network {
        fenced: true,
        ..Network::default()
    });
    let project = written(&[an_installed("solver", vec![solver])], &BTreeMap::new());
    let fence = entry(&project, "solver-fence");

    assert!(sequence(&fence, "command").is_empty());
    assert!(fence.get("dns_search").is_none());
    assert_eq!(environment(&fence, crate::fence::NETWORKS), "");
    assert_eq!(environment(&fence, crate::fence::NAMES), "");
}

/// Nothing sharing the namespace of a service held inside the stack holds what marks a
/// socket: every entry running inside its fence's network, and every entry inside one of
/// those, is written with neither `NET_ADMIN` nor `NET_RAW`, which the fence alone holds.
#[test]
fn no_entry_sharing_a_stack_only_namespace_but_the_fence_can_mark_a_socket() {
    let installed = [held_inside("dashy")];
    let networks = answered(&written(&installed, &BTreeMap::new()), "dashy-fence");
    let project = written(&installed, &networks);
    let mut sharing = vec!["dashy-fence".to_owned()];
    let mut at = 0;
    while let Some(host) = sharing.get(at).cloned() {
        sharing.extend(
            project
                .topology
                .placed
                .iter()
                .filter(|(_, placed)| matches!(placed, Placed::Inside(inside) if *inside == host))
                .map(|(id, _)| id.clone()),
        );
        at += 1;
    }
    assert!(sharing.contains(&"dashy".to_owned()), "{sharing:?}");
    for id in &sharing {
        let granted = sequence(&entry(&project, id), "cap_add");
        let marking: Vec<&String> = granted
            .iter()
            .filter(|cap| *cap == "NET_RAW" || (*cap == "NET_ADMIN" && id != "dashy-fence"))
            .collect();
        assert!(marking.is_empty(), "{id} holds {marking:?}");
    }
    assert_eq!(
        sequence(&entry(&project, "dashy-fence"), "cap_add"),
        ["NET_ADMIN"]
    );
}
