//! Reading the services and host ports a stack's Compose files declare.

use std::path::PathBuf;

use super::read;

/// What `text` declares, with `LAN_PORT` set to 9000 and nothing else set.
fn declared(text: &str) -> (Vec<String>, Vec<(u16, String)>) {
    let (services, ports) = read(
        &[(PathBuf::from("compose.yml"), text.to_owned())],
        &|name| (name == "LAN_PORT").then(|| "9000".to_owned()),
    );
    (
        services.into_iter().map(|one| one.service).collect(),
        ports
            .into_iter()
            .map(|one| (one.port, one.service))
            .collect(),
    )
}

#[test]
fn every_service_is_read_by_name() {
    let (services, _) = declared("services:\n  a: {}\n  b:\n    image: x\n");
    assert_eq!(services, ["a", "b"]);
}

/// The host side is read in every form a mapping is written in, and only the host side.
#[test]
fn the_host_side_of_every_mapping_is_read() {
    let (_, ports) = declared(concat!(
        "services:\n",
        "  a:\n",
        "    ports:\n",
        "      - \"${LAN_BIND:-0.0.0.0}:80:8080\"\n",
        "      - \"127.0.0.1:9696:9696\"\n",
        "      - \"5055:5055\"\n",
        "      - \"[::1]:7000:7000\"\n",
        "      - \"${LAN_PORT:-1}:1\"\n",
        "      - \"${UNSET:-8443}:443\"\n",
        "      - \"6000-6002:6000-6002\"\n",
        "      - published: 4533\n",
        "        target: 4533\n",
        "      - published: \"3000\"\n",
        "        target: 3000\n",
    ));
    let found: Vec<u16> = ports.into_iter().map(|(port, _)| port).collect();
    assert_eq!(
        found,
        [80, 9696, 5055, 7000, 9000, 8443, 6000, 6001, 6002, 4533, 3000]
    );
}

/// What publishes no host port of its own over TCP publishes nothing a plugin can meet.
#[test]
fn a_container_port_alone_udp_and_an_unresolved_port_are_nothing() {
    let (_, ports) = declared(concat!(
        "services:\n",
        "  a:\n",
        "    ports:\n",
        "      - \"8080\"\n",
        "      - 9090\n",
        "      - \"51820:51820/udp\"\n",
        "      - published: 53\n",
        "        protocol: udp\n",
        "      - \"${UNSET}:1\"\n",
        "      - \"7000-6000:1\"\n",
        "      - target: 1\n",
    ));
    assert!(ports.is_empty(), "{ports:?}");
}

/// A file that is not a Compose file declares nothing.
#[test]
fn a_file_that_is_not_compose_declares_nothing() {
    assert_eq!(declared("not: [valid"), (Vec::new(), Vec::new()));
    assert_eq!(declared("name: x\n"), (Vec::new(), Vec::new()));
}

/// Anchors are applied before reading, as Compose applies them.
#[test]
fn anchors_are_applied_first() {
    let (_, ports) = declared(concat!(
        "x-published: &published\n",
        "  ports: [\"8096:8096\"]\n",
        "services:\n",
        "  a:\n",
        "    <<: *published\n",
    ));
    assert_eq!(ports, [(8096, "a".to_owned())]);
}
