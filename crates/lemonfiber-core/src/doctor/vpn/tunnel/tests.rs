use std::sync::Arc;

use lemonfiber_fixtures::support::Reporting;

use super::Gluetun;
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::service::{Failure, Tunnelling};
use crate::test_support::a_guard_forwarding;

const GUARD: &str = "gluetun";

fn guard(engine: Arc<Reporting>) -> Gluetun {
    Gluetun::new(engine, "lemonfiber".to_owned(), GUARD.to_owned(), true)
}

#[tokio::test]
async fn the_port_in_its_status_file_is_the_one_forwarded() {
    assert_eq!(
        guard(a_guard_forwarding(Some("51413")))
            .forwarded()
            .await
            .ok(),
        Some(Some(51413))
    );
}

#[tokio::test]
async fn a_released_port_is_none_forwarded() {
    assert_eq!(
        guard(a_guard_forwarding(Some("0"))).forwarded().await.ok(),
        Some(None)
    );
}

#[tokio::test]
async fn a_guard_that_is_not_running_cannot_tell() {
    let stopped = Reporting::holding(&[GUARD], Lifecycle::Exited, Health::None);
    assert!(matches!(
        guard(Arc::new(stopped)).forwarded().await,
        Err(Failure::Unavailable { .. })
    ));
    assert!(matches!(
        guard(Arc::new(Reporting::absent())).forwarded().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn its_tunnel_is_up_while_it_runs_and_is_not_failing() {
    assert_eq!(guard(a_guard_forwarding(None)).up().await.ok(), Some(true));
    let failing = Reporting::holding(&[GUARD], Lifecycle::Running, Health::Unhealthy);
    assert_eq!(guard(Arc::new(failing)).up().await.ok(), Some(false));
    assert!(guard(Arc::new(Reporting::absent())).up().await.is_err());
}

#[tokio::test]
async fn a_guard_never_asked_to_forward_forwards_none() {
    let unasked = Gluetun::new(
        a_guard_forwarding(Some("51413")),
        "lemonfiber".to_owned(),
        GUARD.to_owned(),
        false,
    );
    assert_eq!(unasked.forwarded().await.ok(), Some(None));
}
