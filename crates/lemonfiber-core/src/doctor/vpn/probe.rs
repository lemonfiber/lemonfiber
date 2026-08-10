//! Asking a container a question, from inside its own network namespace.
//!
//! The whole check rests on this: a public address read from the host tells you
//! about the host, and one read from inside the container tells you about the
//! container. Every question the check asks is asked this way, so they are
//! gathered here — the commands, the containers they run in, and how a silence
//! is read.

use super::echo::Seen;
use super::leak::{is_country_code, looks_like_ip, Reach};
use super::port_forward::{parse_grant, Grant};
use super::FORWARDED_PORT_FILE;
use crate::ports::docker::{Container, Engine, Lifecycle};

/// The command that asks an endpoint for the caller's address, run inside a
/// container.
pub(super) fn wget(url: String) -> Vec<String> {
    vec!["wget".to_owned(), "-qO-".to_owned(), url]
}

/// The command that reads the gateway's forwarded-port status file from inside
/// the container.
pub(super) fn read_port() -> Vec<String> {
    vec!["cat".to_owned(), FORWARDED_PORT_FILE.to_owned()]
}

/// The container implementing a service, where it is present.
pub(super) fn find<'a>(containers: &'a [Container], service: &str) -> Option<&'a Container> {
    containers
        .iter()
        .find(|container| container.service == service)
}

/// A container that is actually running, which is the only kind worth asking
/// anything of.
pub(super) fn running(container: Option<&Container>) -> Option<&Container> {
    container.filter(|container| container.lifecycle == Lifecycle::Running)
}

/// namespace — the exec-based read the leak check and the dashboard's VPN panel
/// share, so both ask the same way. A container that is absent or not running is
/// `Down`; an engine that will not answer is `Unknown`; anything that is not an
/// address is `Blocked`.
/// The public address as every configured source sees it.
///
/// Asked of each in turn from inside the container, because the whole leak check
/// rests on this one number and asking a single stranger makes the guarantee
/// exactly as good as that stranger. A source that will not answer contributes
/// nothing rather than a contradiction.
///
/// The container's reachability is read from the first source that had anything
/// to say about it: down is down whoever was asked, and a container that could
/// not be exec'd into does not become reachable by asking again.
pub(super) async fn addresses(
    engine: &dyn Engine,
    container: Option<&Container>,
    echoes: &[String],
) -> (Reach, Seen) {
    let mut heard: Vec<Option<String>> = Vec::new();
    let mut reach = Reach::Unknown;
    for echo in echoes {
        let answer = public_address(engine, container, echo).await;
        heard.push(match &answer {
            Reach::Address(address) => Some(address.clone()),
            Reach::Blocked | Reach::Down | Reach::Unknown => None,
        });
        // The first definite answer about the container itself stands: a later
        // source failing says something about that source, not about the container.
        if matches!(reach, Reach::Unknown) || matches!(answer, Reach::Address(_)) {
            reach = answer;
        }
    }
    let seen = Seen::of(&heard);
    // Sources that contradict each other leave no address anybody can rely on, so
    // the reachability is downgraded to match rather than carrying one of them.
    let reach = match (&seen, reach) {
        (Seen::Disagreed(_), _) => Reach::Blocked,
        (_, reach) => reach,
    };
    (reach, seen)
}

pub(super) async fn public_address(
    engine: &dyn Engine,
    container: Option<&Container>,
    echo: &str,
) -> Reach {
    let Some(container) = container else {
        return Reach::Down;
    };
    if container.lifecycle != Lifecycle::Running {
        return Reach::Down;
    }
    match engine.exec(&container.id, &wget(echo.to_owned())).await {
        Err(_) => Reach::Unknown,
        Ok(output) => {
            let body = output.stdout.trim();
            if output.status == Some(0) && looks_like_ip(body) {
                Reach::Address(body.to_owned())
            } else {
                Reach::Blocked
            }
        }
    }
}

/// The tunnel's exit country, best effort — reported where the endpoint supplies
/// it, omitted rather than guessed where it cannot.
pub(super) async fn exit_country(
    engine: &dyn Engine,
    container: &Container,
    echo: &str,
) -> Option<String> {
    let url = format!("{}/country-iso", echo.trim_end_matches('/'));
    match engine.exec(&container.id, &wget(url)).await {
        Ok(output) if output.status == Some(0) && is_country_code(output.stdout.trim()) => {
            Some(output.stdout.trim().to_ascii_uppercase())
        }
        _ => None,
    }
}

/// Read the granted port from the gateway's own status file. A container that is
/// not running, or an engine that cannot be reached, makes the answer unknown
/// rather than absent.
pub(super) async fn read_grant(engine: &dyn Engine, gateway: Option<&Container>) -> Grant {
    let Some(container) = gateway else {
        return Grant::Unreadable;
    };
    if container.lifecycle != Lifecycle::Running {
        return Grant::Unreadable;
    }
    // Awaited into a plain value first: a block whose last statement is an await
    // leaves its own closing brace unmarked by coverage.
    let result = engine.exec(&container.id, &read_port()).await;
    match result {
        Err(_) => Grant::Unreadable,
        Ok(output) => parse_grant(&output.stdout),
    }
}
