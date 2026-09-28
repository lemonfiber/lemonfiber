//! Serving encrypted, from the flag that asks for it to the record pairing reads.

use std::process::ExitCode;
use std::sync::Arc;

use lemonfiber_core::companion::served::{last, Served};
use lemonfiber_core::config::Settings;
use lemonfiber_fixtures::ports::Idle;

use super::{a_directory, enough, running, started, Asked};

/// A port nothing holds at the moment of asking, named so a run can be asked for it.
async fn a_free_port() -> Option<u16> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.ok()?;
    listener.local_addr().ok().map(|at| at.port())
}

/// A run keeping what pairing needs at `directory`.
fn keeping_pairing(directory: std::path::PathBuf) -> lemonfiber_core::app::Ctx {
    running(
        Arc::new(Idle),
        Some(enough()),
        Settings {
            companion: Some(directory),
            ..Settings::default()
        },
    )
}

/// Asked to encrypt on a port it names, it serves, and writes down the port and that it
/// encrypted — which is what pairing reads back.
#[tokio::test]
async fn a_run_asked_to_encrypt_serves_and_writes_down_where() {
    let dir = a_directory("encrypted-served");
    let port = a_free_port().await;
    let ended = started(
        keeping_pairing(dir.clone()),
        Asked {
            tls: true,
            port,
            ..Asked::default()
        },
    )
    .await;
    assert_eq!(ended, crate::exit::shown(ExitCode::SUCCESS));
    assert_eq!(
        last(&dir),
        port.map(|port| Served {
            port,
            encrypted: true,
            network: false,
        })
    );
}

/// Asked to encrypt with no port named, it serves nothing: a phone keeps the address it
/// was given.
#[tokio::test]
async fn a_run_asked_to_encrypt_on_no_port_serves_nothing() {
    let dir = a_directory("encrypted-portless");
    let ended = started(
        keeping_pairing(dir.clone()),
        Asked {
            tls: true,
            ..Asked::default()
        },
    )
    .await;
    assert_ne!(ended, crate::exit::shown(ExitCode::SUCCESS));
    assert_eq!(last(&dir), None, "and writes nothing down");
}

/// Where how it is served cannot be written down, it still serves: what cannot be done
/// is pairing, which says so for itself.
#[tokio::test]
async fn a_run_that_cannot_write_down_where_it_serves_still_serves() {
    let dir = a_directory("encrypted-unwritable");
    let _ = std::fs::create_dir_all(&dir);
    let blocked = dir.join("blocked");
    let _ = std::fs::write(&blocked, "a file where the directory would go");
    let ended = started(keeping_pairing(blocked), Asked::default()).await;
    assert_eq!(ended, crate::exit::shown(ExitCode::SUCCESS));
}
