use super::{fetches, reached_at, Service};

fn kinds(of: &[&str]) -> Vec<String> {
    of.iter().map(|kind| (*kind).to_owned()).collect()
}

/// One container in a stack, with only the fields this module reads set to
/// anything meaningful.
fn a_service(id: &str, port: Option<u16>) -> Service {
    Service {
        id: id.to_owned(),
        name: format!("{id} the app"),
        profile: "media".to_owned(),
        image: "example/image".to_owned(),
        tag: "1".to_owned(),
        port,
        bind: None,
        health: None,
        api: None,
        criticality: lemonfiber_manifest::Criticality::Core,
        license: "MIT".to_owned(),
        upstream: "https://example.test".to_owned(),
        last_release: "2026-01-01".to_owned(),
        describes: "an example service".to_owned(),
        without_it: "nothing works".to_owned(),
        media_types: Vec::new(),
        provides: Vec::new(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        asks_for: None,
        reaches: None,
    }
}

/// Which \*arr is a request target, and which is not one at all.
///
/// The third case is the one that carries the requirement: music and books are
/// filed by \*arrs the request service cannot fetch through, so they are not
/// offered as somewhere to send a request. Asserting only the first two would
/// pass a version that offered every \*arr it found.
#[test]
fn only_the_arrs_that_fetch_film_and_television_are_targets() {
    assert_eq!(fetches(&kinds(&["tv"])), Some(true), "television");
    assert_eq!(fetches(&kinds(&["movies"])), Some(false), "film");
    assert_eq!(
        fetches(&kinds(&["music"])),
        None,
        "music is not requestable"
    );
    assert_eq!(
        fetches(&kinds(&["books"])),
        None,
        "books are not requestable"
    );
    assert_eq!(fetches(&[]), None, "an *arr filing nothing is not a target");
}

/// The request service reaches an \*arr by container name, not by loopback.
///
/// It is a container itself, so `127.0.0.1` there is the request service rather
/// than the \*arr — an address that resolves and answers, wrongly.
#[test]
fn an_arr_is_named_by_its_container_and_port() {
    let services = vec![a_service("sonarr", Some(8989))];

    assert_eq!(
        reached_at(&services, "sonarr"),
        Some(("sonarr".to_owned(), 8989))
    );
    assert_eq!(
        reached_at(&services, "radarr"),
        None,
        "an *arr that is not in the stack has nowhere to be reached"
    );
    assert_eq!(
        reached_at(&[a_service("sonarr", None)], "sonarr"),
        None,
        "an *arr publishing no port has no endpoint to hand over"
    );
}
