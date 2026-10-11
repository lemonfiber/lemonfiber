use lemonfiber_manifest::Criticality;

use super::manifest;
use crate::config::Protocols;
use crate::docker::{stopping_order, survey, Halted, Service, State};

/// One service depending on another, for the ordering tests.
fn depending(id: &str, on: &[&str]) -> Service {
    Service {
        id: id.to_owned(),
        name: id.to_owned(),
        describes: format!("what {id} is for"),
        profile: "torrent".to_owned(),
        forms: Vec::new(),
        state: State::Healthy,
        criticality: Criticality::Core,
        depends_on: on.iter().map(|id| (*id).to_owned()).collect(),
        exit: None,
    }
}

/// The case the requirement is about: the client shares the tunnel's network, so
/// the tunnel going first would take the client's network out from under it.
#[test]
fn the_tunnel_goes_down_after_what_is_using_its_network() {
    let running = vec![
        depending("gluetun", &[]),
        depending("qbittorrent", &["gluetun"]),
    ];
    assert_eq!(
        stopping_order(&running, &["gluetun".to_owned(), "qbittorrent".to_owned()]),
        vec!["qbittorrent".to_owned(), "gluetun".to_owned()],
        "named the other way round, and still stopped in this one"
    );
}

/// Read from what each service declares rather than from a rule about VPNs, so a
/// stack that adds another such pair gets the same treatment.
#[test]
fn anything_depended_on_goes_last_whatever_it_is() {
    let running = vec![
        depending("under", &[]),
        depending("over", &["under"]),
        depending("above", &["over"]),
    ];
    assert_eq!(
        stopping_order(
            &running,
            &["under".to_owned(), "over".to_owned(), "above".to_owned()]
        ),
        vec!["above".to_owned(), "over".to_owned(), "under".to_owned()],
        "each one goes before the thing it needs"
    );
}

/// The same request must produce the same command, or a golden file tests nothing.
#[test]
fn services_that_do_not_depend_on_each_other_are_ordered_by_name() {
    let running = vec![
        depending("sonarr", &[]),
        depending("radarr", &[]),
        depending("bazarr", &[]),
    ];
    assert_eq!(
        stopping_order(
            &running,
            &[
                "sonarr".to_owned(),
                "bazarr".to_owned(),
                "radarr".to_owned()
            ]
        ),
        vec![
            "bazarr".to_owned(),
            "radarr".to_owned(),
            "sonarr".to_owned()
        ]
    );
}

/// A circle has no order that satisfies it. Stopping them in one nobody can fault
/// beats refusing to stop them at all.
#[test]
fn services_that_need_each_other_are_still_stopped() {
    let running = vec![depending("one", &["other"]), depending("other", &["one"])];
    let order = stopping_order(&running, &["one".to_owned(), "other".to_owned()]);
    assert_eq!(order.len(), 2, "both are stopped: {order:?}");
    assert!(order.contains(&"one".to_owned()) && order.contains(&"other".to_owned()));
}

/// A service depending on something that is staying up is not held back by it.
#[test]
fn a_dependency_that_is_not_being_stopped_does_not_hold_anything_back() {
    let running = vec![
        depending("gluetun", &[]),
        depending("qbittorrent", &["gluetun"]),
    ];
    assert_eq!(
        stopping_order(&running, &["qbittorrent".to_owned()]),
        vec!["qbittorrent".to_owned()],
        "only what was asked for is stopped, and nothing waits on the rest"
    );
}

/// A service that runs inside another's network depends on that one, read from what
/// its `network.via` names rather than from either service's name.
#[test]
fn a_service_running_inside_another_s_network_depends_on_it() {
    let profiles = ["torrent".to_owned()];
    let surveyed = manifest()
        .map(|manifest| {
            survey(
                &manifest,
                &profiles,
                &[],
                &Halted::default(),
                Protocols::both(),
            )
        })
        .unwrap_or_default();
    let needs = |id: &str| {
        surveyed
            .iter()
            .find(|service| service.id == id)
            .map(|service| service.depends_on.clone())
    };

    assert_eq!(needs("qbittorrent"), Some(vec!["gluetun".to_owned()]));
    assert_eq!(needs("gluetun"), Some(Vec::new()));
}
