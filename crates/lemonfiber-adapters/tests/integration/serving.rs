//! Serving an image as its service runs, driven against an engine written for it: a
//! network of its own, the container on it with its port on loopback, read, and both
//! removed.

use crate::fake;
#[cfg(unix)]
use lemonfiber_adapters::Daemon;
use lemonfiber_ports::program::Unran;
use lemonfiber_ports::serving::{Serve, Serving as _, Standing};

fn serve() -> Serve {
    Serve {
        reaching: Vec::new(),
        image: "lscr.io/linuxserver/sonarr@sha256:dffc".to_owned(),
        env: vec!["TZ=UTC".to_owned()],
        user: None,
        security_opt: vec!["no-new-privileges:true".to_owned()],
        cap_drop: vec!["ALL".to_owned()],
        cap_add: Vec::new(),
        pids_limit: 1024,
        read_only: false,
        memory: None,
        binds: Vec::new(),
        tmpfs: Vec::new(),
        listens: Some(8989),
        alias: None,
    }
}

const INSPECTED: &str = r#"{
    "Id": "s1",
    "Config": {"Hostname": "s1short"},
    "State": {"Running": true, "ExitCode": 0, "Health": {"Status": "healthy"}},
    "NetworkSettings": {
        "Ports": {"8989/tcp": [{"HostIp": "127.0.0.1", "HostPort": "49153"}]},
        "Networks": {"lemonfiber-record": {"IPAddress": "172.18.0.2", "MacAddress": "02:42:ac:12:00:02"}}
    }
}"#;

#[cfg(unix)]
#[tokio::test]
async fn a_service_is_served_on_a_network_of_its_own_read_and_removed_with_it() {
    let mut engine = fake::engine(
        "serving",
        vec![
            ("/containers/json", fake::Reply::Body(200, "[]".to_owned())),
            (
                "/info",
                fake::Reply::Body(
                    200,
                    r#"{"Name":"recording-host","KernelVersion":"7.0.14","OperatingSystem":"OrbStack"}"#
                        .to_owned(),
                ),
            ),
            (
                "/networks/create",
                fake::Reply::Body(201, r#"{"Id":"n1","Warning":""}"#.to_owned()),
            ),
            (
                "/networks?",
                fake::Reply::Body(
                    200,
                    r#"[{"Name":"lemonfiber-record-1-1-0"},{"Name":"bridge"}]"#.to_owned(),
                ),
            ),
            ("/networks/", fake::Reply::Body(204, String::new())),
            (
                "/containers/create",
                fake::Reply::Body(201, r#"{"Id":"s1","Warnings":[]}"#.to_owned()),
            ),
            (
                "/containers/s1/start",
                fake::Reply::Body(204, String::new()),
            ),
            (
                "/containers/s1/json",
                fake::Reply::Body(200, INSPECTED.to_owned()),
            ),
            (
                "/containers/s1/logs",
                fake::Reply::Multiplexed(vec![
                    (1, "started\n".to_owned()),
                    (0, "typed in\n".to_owned()),
                    (2, "a warning\n".to_owned()),
                ]),
            ),
            ("/containers/s1?", fake::Reply::Body(204, String::new())),
        ],
    );
    let daemon = Daemon::at(&engine.socket);

    let served = daemon.serve(&serve()).await;
    assert!(served.is_ok(), "{served:?}");
    let Ok(served) = served else {
        return;
    };
    assert_eq!(served.container, "s1");
    assert_eq!(served.hostname, "s1short");
    assert_eq!(served.port, Some(49153));
    assert_eq!(
        served.addresses,
        vec!["172.18.0.2".to_owned(), "02:42:ac:12:00:02".to_owned()]
    );
    assert!(served.network.starts_with("lemonfiber-record-"));
    assert_eq!(
        served.machine,
        vec![
            "recording-host".to_owned(),
            "7.0.14".to_owned(),
            "OrbStack".to_owned()
        ]
    );
    assert_eq!(daemon.standing(&served).await, Ok(Standing::Healthy));
    assert_eq!(daemon.log(&served, 50).await, "started\na warning\n");
    daemon.removed(&served).await;

    let asked = engine.asked_for();
    assert!(
        asked
            .iter()
            .any(|path| path.starts_with("/networks/lemonfiber-record-1-1-0")),
        "a network an earlier recording left is removed: {asked:?}"
    );
    assert!(
        asked
            .iter()
            .any(|path| path.starts_with("/containers/s1?") && path.contains("force=true")),
        "{asked:?}"
    );
    assert!(
        asked
            .iter()
            .any(|path| path.starts_with(&format!("/networks/{}", served.network))),
        "{asked:?}"
    );
    assert!(
        asked
            .iter()
            .any(|path| path.contains("/logs?") && path.contains("tail=50")),
        "{asked:?}"
    );
    assert!(!asked.iter().any(|path| path.contains("/images/create")));
    engine.stop().await;
}

fn served_on(network: &str) -> lemonfiber_ports::serving::Served {
    lemonfiber_ports::serving::Served {
        container: "u1".to_owned(),
        network: network.to_owned(),
        hostname: "u1short".to_owned(),
        addresses: Vec::new(),
        machine: vec!["recording-host".to_owned()],
        port: Some(49152),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_container_beside_another_joins_its_network_and_leaves_the_network_to_it() {
    let mut engine = fake::engine(
        "serving-beside",
        vec![
            (
                "/containers/create",
                fake::Reply::Body(201, r#"{"Id":"s1","Warnings":[]}"#.to_owned()),
            ),
            (
                "/containers/s1/start",
                fake::Reply::Body(204, String::new()),
            ),
            (
                "/containers/s1/json",
                fake::Reply::Body(200, INSPECTED.to_owned()),
            ),
        ],
    );
    let beside = Daemon::at(&engine.socket)
        .beside(&served_on("lemonfiber-record-9-9-9"), &serve())
        .await;
    assert!(beside.is_ok(), "{beside:?}");
    let Ok(beside) = beside else {
        return;
    };
    assert_eq!(beside.container, "s1");
    assert_eq!(beside.network, "lemonfiber-record-9-9-9");
    assert_eq!(beside.machine, vec!["recording-host".to_owned()]);
    assert_eq!(beside.port, Some(49153));
    let asked = engine.asked_for();
    assert!(
        !asked
            .iter()
            .any(|path| path.starts_with("/networks") || path.starts_with("/info")),
        "{asked:?}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_container_beside_another_that_cannot_be_made_leaves_the_network_alone() {
    let mut engine = fake::engine(
        "serving-beside-no-image",
        vec![(
            "/containers/create",
            fake::Reply::Body(
                404,
                r#"{"message":"No such image: lscr.io/linuxserver/sonarr@sha256:dffc"}"#.to_owned(),
            ),
        )],
    );
    let beside = Daemon::at(&engine.socket)
        .beside(&served_on("lemonfiber-record-9-9-9"), &serve())
        .await;
    assert_eq!(
        beside,
        Err(Unran::NoImage {
            image: "lscr.io/linuxserver/sonarr@sha256:dffc".to_owned()
        })
    );
    assert!(!engine
        .asked_for()
        .iter()
        .any(|path| path.starts_with("/networks")));
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn an_image_the_engine_does_not_hold_leaves_neither_container_nor_network() {
    let mut engine = fake::engine(
        "serving-no-image",
        vec![
            ("/containers/json", fake::Reply::Body(200, "[]".to_owned())),
            (
                "/info",
                fake::Reply::Body(
                    200,
                    r#"{"Name":"recording-host","KernelVersion":"7.0.14","OperatingSystem":"OrbStack"}"#
                        .to_owned(),
                ),
            ),
            (
                "/networks/create",
                fake::Reply::Body(201, r#"{"Id":"n1","Warning":""}"#.to_owned()),
            ),
            ("/networks?", fake::Reply::Body(500, r#"{"message":"no"}"#.to_owned())),
            ("/networks/", fake::Reply::Body(204, String::new())),
            (
                "/containers/create",
                fake::Reply::Body(
                    404,
                    r#"{"message":"No such image: lscr.io/linuxserver/sonarr@sha256:dffc"}"#
                        .to_owned(),
                ),
            ),
        ],
    );

    let served = Daemon::at(&engine.socket).serve(&serve()).await;

    assert_eq!(
        served,
        Err(Unran::NoImage {
            image: "lscr.io/linuxserver/sonarr@sha256:dffc".to_owned()
        })
    );
    let asked = engine.asked_for();
    assert!(
        asked
            .iter()
            .any(|path| path.starts_with("/networks/lemonfiber-record-")),
        "the network made for it is removed: {asked:?}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_container_that_will_not_start_is_refused_and_removed_with_its_network() {
    let mut engine = fake::engine(
        "serving-unstarted",
        vec![
            ("/containers/json", fake::Reply::Body(200, "[]".to_owned())),
            (
                "/info",
                fake::Reply::Body(
                    200,
                    r#"{"Name":"recording-host","KernelVersion":"7.0.14","OperatingSystem":"OrbStack"}"#
                        .to_owned(),
                ),
            ),
            (
                "/networks/create",
                fake::Reply::Body(201, r#"{"Id":"n1","Warning":""}"#.to_owned()),
            ),
            ("/networks?", fake::Reply::Body(200, "[]".to_owned())),
            ("/networks/", fake::Reply::Body(204, String::new())),
            (
                "/containers/create",
                fake::Reply::Body(201, r#"{"Id":"s1","Warnings":[]}"#.to_owned()),
            ),
            (
                "/containers/s1/start",
                fake::Reply::Body(500, r#"{"message":"port is already allocated"}"#.to_owned()),
            ),
            ("/containers/s1?", fake::Reply::Body(204, String::new())),
        ],
    );

    let served = Daemon::at(&engine.socket).serve(&serve()).await;

    assert!(
        matches!(&served, Err(Unran::Refused { reason }) if reason.contains("already allocated")),
        "{served:?}"
    );
    let asked = engine.asked_for();
    assert!(asked.iter().any(|path| path.starts_with("/containers/s1?")));
    assert!(asked
        .iter()
        .any(|path| path.starts_with("/networks/lemonfiber-record-")));
    engine.stop().await;
}

#[cfg(unix)]
fn info() -> (&'static str, fake::Reply) {
    (
        "/info",
        fake::Reply::Body(
            200,
            r#"{"Name":"recording-host","KernelVersion":"7.0.14","OperatingSystem":"OrbStack"}"#
                .to_owned(),
        ),
    )
}

#[cfg(unix)]
#[tokio::test]
async fn a_network_the_engine_will_not_make_serves_nothing() {
    let mut engine = fake::engine(
        "serving-no-network",
        vec![
            ("/containers/json", fake::Reply::Body(200, "[]".to_owned())),
            info(),
            ("/networks?", fake::Reply::Body(200, "[]".to_owned())),
            (
                "/networks/create",
                fake::Reply::Body(500, r#"{"message":"address pools exhausted"}"#.to_owned()),
            ),
        ],
    );

    let served = Daemon::at(&engine.socket).serve(&serve()).await;

    assert!(
        matches!(&served, Err(Unran::Refused { reason }) if reason.contains("address pools exhausted")),
        "{served:?}"
    );
    assert!(!engine
        .asked_for()
        .iter()
        .any(|path| path.contains("/containers/create")));
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_started_container_the_engine_will_not_describe_is_refused_and_removed() {
    let mut engine = fake::engine(
        "serving-undescribed",
        vec![
            ("/containers/json", fake::Reply::Body(200, "[]".to_owned())),
            info(),
            (
                "/networks/create",
                fake::Reply::Body(201, r#"{"Id":"n1","Warning":""}"#.to_owned()),
            ),
            ("/networks?", fake::Reply::Body(200, "[]".to_owned())),
            ("/networks/", fake::Reply::Body(204, String::new())),
            (
                "/containers/create",
                fake::Reply::Body(201, r#"{"Id":"s1","Warnings":[]}"#.to_owned()),
            ),
            (
                "/containers/s1/start",
                fake::Reply::Body(204, String::new()),
            ),
            (
                "/containers/s1/json",
                fake::Reply::Body(500, r#"{"message":"no such container"}"#.to_owned()),
            ),
            ("/containers/s1?", fake::Reply::Body(204, String::new())),
        ],
    );
    let daemon = Daemon::at(&engine.socket);

    let served = daemon.serve(&serve()).await;

    assert!(
        matches!(&served, Err(Unran::Refused { reason }) if reason.contains("no such container")),
        "{served:?}"
    );
    let somebody = lemonfiber_ports::serving::Served {
        container: "s1".to_owned(),
        network: "n".to_owned(),
        hostname: "h".to_owned(),
        addresses: Vec::new(),
        machine: Vec::new(),
        port: None,
    };
    assert!(matches!(
        daemon.standing(&somebody).await,
        Err(Unran::Refused { reason }) if reason.contains("no such container")
    ));
    let asked = engine.asked_for();
    assert!(asked.iter().any(|path| path.starts_with("/containers/s1?")));
    assert!(asked
        .iter()
        .any(|path| path.starts_with("/networks/lemonfiber-record-")));
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn an_engine_that_will_not_say_what_machine_it_runs_on_serves_nothing() {
    let mut engine = fake::engine(
        "serving-no-info",
        vec![(
            "/info",
            fake::Reply::Body(500, r#"{"message":"no"}"#.to_owned()),
        )],
    );

    let served = Daemon::at(&engine.socket).serve(&serve()).await;

    assert!(matches!(served, Err(Unran::Refused { .. })), "{served:?}");
    let asked = engine.asked_for();
    assert!(
        !asked
            .iter()
            .any(|path| path.contains("/networks/create") || path.contains("/containers/create")),
        "{asked:?}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn an_engine_that_cannot_be_reached_serves_nothing_says_nothing_and_removes_nothing() {
    let nowhere = std::env::temp_dir().join("lemonfiber-no-engine-here.sock");
    let daemon = Daemon::at(&nowhere);
    let served = daemon.serve(&serve()).await;
    assert!(
        matches!(served, Err(Unran::Unreachable { .. })),
        "{served:?}"
    );
    let somebody = lemonfiber_ports::serving::Served {
        container: "c".to_owned(),
        network: "n".to_owned(),
        hostname: "h".to_owned(),
        addresses: Vec::new(),
        machine: Vec::new(),
        port: None,
    };
    assert!(matches!(
        daemon.standing(&somebody).await,
        Err(Unran::Unreachable { .. })
    ));
    assert_eq!(daemon.log(&somebody, 50).await, "");
    daemon.removed(&somebody).await;
}
