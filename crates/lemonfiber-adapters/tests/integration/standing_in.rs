//! A stand-in answering on the address the engine's containers reach this machine by,
//! driven against an engine written for it and asked over a real connection.

#![cfg(unix)]

use std::time::Duration;

use lemonfiber_adapters::Daemon;
use lemonfiber_ports::program::Unran;
use lemonfiber_ports::serving::{Said, Serving as _, StandIn};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

use crate::fake;

const VERSION: &str = r#"{"version":"4.5.1"}"#;

fn a_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|taken| taken.local_addr())
        .map(|at| at.port())
        .unwrap_or_default()
}

fn stand_in(port: u16) -> StandIn {
    StandIn {
        port,
        headers: Vec::new(),
        answers: vec![Said {
            query: Some(("mode".to_owned(), "version".to_owned())),
            ..Said::json("GET", "/api", VERSION.to_owned())
        }],
    }
}

fn info(system: &str) -> (&'static str, fake::Reply) {
    (
        "/info",
        fake::Reply::Body(
            200,
            format!(r#"{{"Name":"recording-host","OperatingSystem":"{system}"}}"#),
        ),
    )
}

fn bridge(config: &str) -> (&'static str, fake::Reply) {
    (
        "/networks/bridge",
        fake::Reply::Body(
            200,
            format!(r#"{{"Name":"bridge","IPAM":{{"Config":{config}}}}}"#),
        ),
    )
}

async fn sent(port: u16, request: &[u8]) -> std::io::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await?;
    stream.write_all(request).await?;
    stream.shutdown().await?;
    let mut said = String::new();
    stream.read_to_string(&mut said).await?;
    Ok(said)
}

async fn asked(port: u16, target: &str) -> std::io::Result<String> {
    sent(
        port,
        format!("GET {target} HTTP/1.1\r\nHost: conformance-download\r\nAccept: */*\r\n\r\n")
            .as_bytes(),
    )
    .await
}

async fn refused_once_dropped(port: u16) -> bool {
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_err() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

#[tokio::test]
async fn under_docker_desktop_it_answers_on_loopback_what_it_was_given_until_dropped() {
    let mut engine = fake::engine("standing-in-desktop", vec![info("Docker Desktop")]);
    let port = a_free_port();

    let answering = Daemon::at(&engine.socket).stand_in(&stand_in(port)).await;

    assert!(answering.is_ok(), "{:?}", answering.as_ref().err());
    let given = asked(port, "/api?apikey=k&mode=version&output=json")
        .await
        .unwrap_or_default();
    assert!(given.starts_with("HTTP/1.1 200 \r\n"), "{given}");
    assert!(
        given.contains("Content-Type: application/json\r\n"),
        "{given}"
    );
    assert!(
        given.contains(&format!("Content-Length: {}\r\n", VERSION.len())),
        "{given}"
    );
    assert!(given.ends_with(&format!("\r\n\r\n{VERSION}")), "{given}");
    let other = asked(port, "/api?mode=shutdown").await.unwrap_or_default();
    assert!(other.starts_with("HTTP/1.1 404 \r\n"), "{other}");
    assert!(other.ends_with(r#"{"status":false,"error":"not answered here"}"#));
    assert!(!engine
        .asked_for()
        .iter()
        .any(|path| path.contains("/networks/bridge")));
    drop(answering);
    assert!(refused_once_dropped(port).await);
    engine.stop().await;
}

#[tokio::test]
async fn a_request_it_cannot_read_is_refused_and_one_cut_short_is_answered_as_far_as_it_went() {
    let engine = fake::engine("standing-in-unread", vec![info("Docker Desktop")]);
    let port = a_free_port();
    let answering = Daemon::at(&engine.socket).stand_in(&stand_in(port)).await;

    let unreadable = sent(
        port,
        b"GET /api?mode=\xffversion HTTP/1.1\r\nHost: \xfe\r\n\r\n",
    )
    .await
    .unwrap_or_default();
    let cut_short = sent(
        port,
        b"GET /api?mode=version HTTP/1.1\r\nHost: conformance-",
    )
    .await
    .unwrap_or_default();

    assert!(answering.is_ok(), "{:?}", answering.as_ref().err());
    assert!(unreadable.starts_with("HTTP/1.1 404 \r\n"), "{unreadable}");
    assert!(cut_short.starts_with("HTTP/1.1 200 \r\n"), "{cut_short}");
    assert!(cut_short.ends_with(VERSION), "{cut_short}");
    drop(answering);
    engine.stop().await;
}

#[tokio::test]
async fn elsewhere_it_answers_on_the_gateway_of_the_engine_bridge_network() {
    let mut engine = fake::engine(
        "standing-in-bridge",
        vec![
            info("OrbStack"),
            bridge(r#"[{"Subnet":"fd00::/64"},{"Subnet":"127.0.0.0/8","Gateway":"127.0.0.1"}]"#),
        ],
    );
    let port = a_free_port();

    let answering = Daemon::at(&engine.socket).stand_in(&stand_in(port)).await;

    assert!(answering.is_ok(), "{:?}", answering.as_ref().err());
    let given = asked(port, "/api?mode=version").await.unwrap_or_default();
    assert!(given.ends_with(VERSION), "{given}");
    assert!(engine
        .asked_for()
        .iter()
        .any(|path| path.contains("/networks/bridge")));
    drop(answering);
    engine.stop().await;
}

#[tokio::test]
async fn a_bridge_network_naming_no_gateway_it_can_read_is_refused() {
    let engine = fake::engine(
        "standing-in-no-gateway",
        vec![
            info("OrbStack"),
            bridge(r#"[{"Gateway":"not an address"}]"#),
        ],
    );

    let answering = Daemon::at(&engine.socket)
        .stand_in(&stand_in(a_free_port()))
        .await;

    assert!(
        matches!(&answering, Err(Unran::Refused { reason }) if reason.contains("no gateway")),
        "{:?}",
        answering.as_ref().err()
    );
    engine.stop().await;
}

#[tokio::test]
async fn an_engine_that_will_not_describe_itself_or_its_bridge_is_refused() {
    let silent = fake::engine(
        "standing-in-no-info",
        vec![(
            "/info",
            fake::Reply::Body(500, r#"{"message":"no info"}"#.to_owned()),
        )],
    );
    let no_bridge = fake::engine("standing-in-no-bridge", vec![info("OrbStack")]);

    let unsaid = Daemon::at(&silent.socket)
        .stand_in(&stand_in(a_free_port()))
        .await;
    let unbridged = Daemon::at(&no_bridge.socket)
        .stand_in(&stand_in(a_free_port()))
        .await;

    assert!(
        matches!(&unsaid, Err(Unran::Refused { reason }) if reason.contains("no info")),
        "{:?}",
        unsaid.as_ref().err()
    );
    assert!(
        matches!(&unbridged, Err(Unran::Refused { reason }) if reason.contains("no such route")),
        "{:?}",
        unbridged.as_ref().err()
    );
    silent.stop().await;
    no_bridge.stop().await;
}

#[tokio::test]
async fn a_port_already_taken_is_refused() {
    let engine = fake::engine("standing-in-taken", vec![info("Docker Desktop")]);
    let taken = std::net::TcpListener::bind("127.0.0.1:0");
    let port = taken
        .as_ref()
        .ok()
        .and_then(|held| held.local_addr().ok())
        .map(|at| at.port())
        .unwrap_or_default();

    let answering = Daemon::at(&engine.socket).stand_in(&stand_in(port)).await;

    assert!(
        matches!(&answering, Err(Unran::Refused { reason }) if reason.contains("could not be taken")),
        "{:?}",
        answering.as_ref().err()
    );
    drop(taken);
    engine.stop().await;
}

#[tokio::test]
async fn an_engine_that_cannot_be_reached_answers_nothing() {
    let nowhere = std::env::temp_dir().join("lemonfiber-no-engine-to-stand-in.sock");

    let answering = Daemon::at(&nowhere)
        .stand_in(&stand_in(a_free_port()))
        .await;

    assert!(
        matches!(answering, Err(Unran::Unreachable { .. })),
        "{:?}",
        answering.as_ref().err()
    );
}

#[tokio::test]
async fn what_one_request_says_is_answered_back_to_a_later_one() {
    let engine = fake::engine("standing-in-heard", vec![info("Docker Desktop")]);
    let port = a_free_port();
    let keys = StandIn {
        port,
        headers: Vec::new(),
        answers: vec![
            Said::empty("POST", "/Auth/Keys"),
            Said::json(
                "GET",
                "/Auth/Keys",
                r#"{"Items":[{"AppName":"{query:App}"}]}"#.to_owned(),
            ),
        ],
    };
    let answering = Daemon::at(&engine.socket).stand_in(&keys).await;

    let made = sent(
        port,
        b"POST /Auth/Keys?App=conformance HTTP/1.1\r\nHost: conformance-identity\r\n\r\n",
    )
    .await
    .unwrap_or_default();
    let listed = asked(port, "/Auth/Keys").await.unwrap_or_default();

    assert!(answering.is_ok(), "{:?}", answering.as_ref().err());
    assert_eq!(made, "HTTP/1.1 204 \r\nConnection: close\r\n\r\n");
    assert!(
        listed.ends_with(r#"{"Items":[{"AppName":"conformance"}]}"#),
        "{listed}"
    );
    drop(answering);
}
