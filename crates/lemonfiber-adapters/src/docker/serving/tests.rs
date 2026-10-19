use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use bollard::models::{
    ContainerConfig, ContainerInspectResponse, ContainerState, EndpointSettings, Health,
    HealthStatusEnum, MountType, NetworkSettings, PortBinding, SystemInfo,
};
use lemonfiber_ports::program::LEFT_BEHIND;
use lemonfiber_ports::serving::{Bound, Serve, Standing};

use super::{body, left_behind, machine, named, read_off, stood, PROOF_LABEL};

fn serve() -> Serve {
    Serve {
        image: "lscr.io/linuxserver/sonarr@sha256:dffc".to_owned(),
        env: vec!["TZ=UTC".to_owned(), "PUID=1000".to_owned()],
        user: None,
        security_opt: vec!["no-new-privileges:true".to_owned()],
        cap_drop: vec!["ALL".to_owned()],
        cap_add: vec!["CHOWN".to_owned()],
        pids_limit: 1024,
        read_only: false,
        memory: Some(300 * 1024 * 1024),
        binds: vec![
            Bound {
                source: PathBuf::from("/tmp/copy/config"),
                target: "/config".to_owned(),
            },
            Bound {
                source: PathBuf::from("/tmp/copy/data"),
                target: "/data".to_owned(),
            },
        ],
        tmpfs: vec!["/run".to_owned()],
        listens: Some(8989),
        alias: None,
    }
}

#[test]
fn a_served_container_is_on_its_own_network_with_its_port_on_loopback_alone() {
    let asked = body(&serve(), "lemonfiber-record-1-2-3");
    assert_eq!(
        asked.image.as_deref(),
        Some("lscr.io/linuxserver/sonarr@sha256:dffc")
    );
    assert_eq!(asked.cmd, None);
    assert_eq!(asked.entrypoint, None);
    assert_eq!(
        asked.env,
        Some(vec!["TZ=UTC".to_owned(), "PUID=1000".to_owned()])
    );
    assert_eq!(asked.exposed_ports, Some(vec!["8989/tcp".to_owned()]));
    assert_eq!(
        asked.labels,
        Some(HashMap::from([(
            PROOF_LABEL.to_owned(),
            "record".to_owned()
        )]))
    );
    let host = asked.host_config.unwrap_or_default();
    assert_eq!(
        host.network_mode.as_deref(),
        Some("lemonfiber-record-1-2-3")
    );
    assert_eq!(
        host.port_bindings,
        Some(HashMap::from([(
            "8989/tcp".to_owned(),
            Some(vec![PortBinding {
                host_ip: Some("127.0.0.1".to_owned()),
                host_port: Some(String::new()),
            }])
        )]))
    );
    assert_eq!(host.publish_all_ports, Some(false));
    assert_eq!(host.privileged, Some(false));
    assert_eq!(host.cap_drop, Some(vec!["ALL".to_owned()]));
    assert_eq!(host.cap_add, Some(vec!["CHOWN".to_owned()]));
    assert_eq!(
        host.security_opt,
        Some(vec!["no-new-privileges:true".to_owned()])
    );
    assert_eq!(host.pids_limit, Some(1024));
    assert_eq!(host.readonly_rootfs, Some(false));
    assert_eq!(host.memory, Some(300 * 1024 * 1024));
    assert_eq!(
        host.tmpfs,
        Some(HashMap::from([("/run".to_owned(), String::new())]))
    );
    let mounts = host.mounts.unwrap_or_default();
    assert_eq!(
        mounts
            .iter()
            .map(|mount| (mount.typ, mount.source.as_deref(), mount.target.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (
                Some(MountType::BIND),
                Some("/tmp/copy/config"),
                Some("/config")
            ),
            (Some(MountType::BIND), Some("/tmp/copy/data"), Some("/data")),
        ]
    );
    assert_eq!(host.devices, None);
    assert_eq!(host.binds, None);
    assert_eq!(asked.networking_config, None);
}

#[test]
fn a_container_with_an_alias_is_reached_by_it_on_its_network() {
    let asked = body(
        &Serve {
            alias: Some("jellyfin".to_owned()),
            ..serve()
        },
        "lemonfiber-record-1-2-3",
    );
    let aliases = asked
        .networking_config
        .and_then(|config| config.endpoints_config)
        .and_then(|endpoints| endpoints.get("lemonfiber-record-1-2-3").cloned())
        .and_then(|endpoint| endpoint.aliases);
    assert_eq!(aliases, Some(vec!["jellyfin".to_owned()]));
}

#[test]
fn a_service_listening_nowhere_publishes_nothing() {
    let quiet = Serve {
        listens: None,
        ..serve()
    };
    let asked = body(&quiet, "n");
    assert_eq!(asked.exposed_ports, None);
    assert_eq!(asked.host_config.and_then(|host| host.port_bindings), None);
}

fn inspected(ports: Vec<PortBinding>) -> ContainerInspectResponse {
    ContainerInspectResponse {
        config: Some(ContainerConfig {
            hostname: Some("4f1c2d3e5a6b".to_owned()),
            ..ContainerConfig::default()
        }),
        network_settings: Some(NetworkSettings {
            ports: Some(HashMap::from([("8989/tcp".to_owned(), Some(ports))])),
            networks: Some(HashMap::from([(
                "lemonfiber-record-1".to_owned(),
                EndpointSettings {
                    ip_address: Some("172.18.0.2".to_owned()),
                    global_ipv6_address: Some(String::new()),
                    mac_address: Some("02:42:ac:12:00:02".to_owned()),
                    ..EndpointSettings::default()
                },
            )])),
            ..NetworkSettings::default()
        }),
        ..ContainerInspectResponse::default()
    }
}

#[test]
fn what_a_started_container_is_known_by_and_where_loopback_reaches_it_is_read_off() {
    let served = read_off(
        &inspected(vec![
            PortBinding {
                host_ip: Some("0.0.0.0".to_owned()),
                host_port: Some("40000".to_owned()),
            },
            PortBinding {
                host_ip: Some("127.0.0.1".to_owned()),
                host_port: Some("49153".to_owned()),
            },
        ]),
        "4f1c2d3e5a6b7c8d",
        "lemonfiber-record-1",
        Some(8989),
    );
    assert_eq!(served.container, "4f1c2d3e5a6b7c8d");
    assert_eq!(served.network, "lemonfiber-record-1");
    assert_eq!(served.hostname, "4f1c2d3e5a6b");
    assert_eq!(
        served.addresses,
        vec!["172.18.0.2".to_owned(), "02:42:ac:12:00:02".to_owned()]
    );
    assert_eq!(served.port, Some(49153));
    let unpublished = read_off(&inspected(Vec::new()), "c", "n", Some(8989));
    assert_eq!(unpublished.port, None);
    let wide_only = read_off(
        &inspected(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_owned()),
            host_port: Some("40000".to_owned()),
        }]),
        "c",
        "n",
        Some(8989),
    );
    assert_eq!(wide_only.port, None, "only the loopback interface is asked");
}

fn state(running: bool, health: Option<HealthStatusEnum>, exit: i64) -> ContainerInspectResponse {
    ContainerInspectResponse {
        state: Some(ContainerState {
            running: Some(running),
            exit_code: Some(exit),
            health: health.map(|status| Health {
                status: Some(status),
                ..Health::default()
            }),
            ..ContainerState::default()
        }),
        ..ContainerInspectResponse::default()
    }
}

#[test]
fn how_a_container_stands_is_its_own_health_where_it_has_one() {
    assert_eq!(stood(&state(true, None, 0)), Standing::Running);
    assert_eq!(
        stood(&state(true, Some(HealthStatusEnum::NONE), 0)),
        Standing::Running
    );
    assert_eq!(
        stood(&state(true, Some(HealthStatusEnum::STARTING), 0)),
        Standing::Starting
    );
    assert_eq!(
        stood(&state(true, Some(HealthStatusEnum::HEALTHY), 0)),
        Standing::Healthy
    );
    assert_eq!(
        stood(&state(true, Some(HealthStatusEnum::UNHEALTHY), 0)),
        Standing::Unhealthy
    );
    assert_eq!(
        stood(&state(false, Some(HealthStatusEnum::HEALTHY), 3)),
        Standing::Exited(3)
    );
    assert_eq!(
        stood(&ContainerInspectResponse::default()),
        Standing::Exited(-1)
    );
}

#[test]
fn only_a_network_named_as_these_are_and_older_than_any_recording_is_left_behind() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
    let fresh = named(now);
    assert!(fresh.starts_with("lemonfiber-record-10000-"), "{fresh}");
    let old = named(now - LEFT_BEHIND - Duration::from_secs(1));
    let edge = named(now - LEFT_BEHIND);
    let names = vec![
        fresh,
        old.clone(),
        edge,
        "lemonfiber-record-x-1-1".to_owned(),
        "bridge".to_owned(),
        "media_default".to_owned(),
    ];
    assert_eq!(left_behind(&names, now), vec![old]);
    assert!(left_behind(&names, SystemTime::UNIX_EPOCH).is_empty());
    let before = SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs(1));
    assert!(before.is_none_or(|before| left_behind(&names, before).is_empty()));
}

#[test]
fn the_machine_is_known_by_each_of_its_name_kernel_and_system_the_engine_names() {
    let info = SystemInfo {
        name: Some("orbstack".to_owned()),
        kernel_version: Some("7.0.14-orbstack".to_owned()),
        operating_system: Some(String::new()),
        ncpu: Some(8),
        ..SystemInfo::default()
    };
    assert_eq!(
        machine(&info),
        vec!["orbstack".to_owned(), "7.0.14-orbstack".to_owned()]
    );
    assert!(machine(&SystemInfo::default()).is_empty());
}
