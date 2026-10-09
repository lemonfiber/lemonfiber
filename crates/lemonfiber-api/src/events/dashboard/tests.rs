use std::sync::Arc;

use lemonfiber_core::alert::{Alert, Moment};
use lemonfiber_core::error::Severity;
use lemonfiber_core::model::kind;

use super::{alerted, playing, Dashboard};

/// One alert about `check`, at its `recurrence`, going `moment`'s way.
fn an_alert(check: &str, recurrence: u32, moment: Moment) -> Alert {
    Alert {
        id: Some(Alert::named(check, recurrence)),
        check: check.to_owned(),
        kind: "service.down".to_owned(),
        moment,
        severity: Severity::Critical,
        summary: format!("{check} stopped"),
        meaning: "nothing is downloading".to_owned(),
        remedies: vec!["start it again".to_owned()],
        affected: vec![check.to_owned()],
        exit: None,
    }
}

fn a_dashboard() -> Dashboard {
    Dashboard::against(Arc::new(lemonfiber_testing::a_context().build()))
}

/// The identity and direction of each thing said, in the order it was said, as a client
/// reads them off the wire.
fn named(said: &[crate::events::wire::Rendered]) -> Vec<String> {
    said.iter()
        .map(|one| {
            let framed = crate::events::wire::Event::placed("1".to_owned(), one.clone()).framed();
            assert!(framed.contains(&format!("event: {}\n", kind::ALERT.as_str())));
            let data: String = framed
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .collect();
            serde_json::from_str::<serde_json::Value>(&data)
                .ok()
                .and_then(|envelope| {
                    let data = envelope.get("data")?;
                    Some(format!(
                        "{} {}",
                        data.get("id")?.as_str()?,
                        data.get("moment")?.as_str()?
                    ))
                })
                .unwrap_or_default()
        })
        .collect()
}

/// What the list carried when the run began was said before anybody here listened, so
/// the first gather says none of it.
#[tokio::test]
async fn the_alerts_already_carried_when_the_run_began_are_not_said() {
    let dashboard = a_dashboard();
    let said = alerted(&dashboard, &[an_alert("sonarr", 1, Moment::Onset)]).await;
    assert!(said.is_empty());
}

/// An alert that starts is said, its resolution is said under the same identity, and the
/// next time the same thing goes wrong is said under a new one — each once.
#[tokio::test]
async fn an_onset_and_its_resolution_are_said_once_each_under_one_identity() {
    let dashboard = a_dashboard();
    let onset = an_alert("sonarr", 1, Moment::Onset);
    let resolved = an_alert("sonarr", 1, Moment::Resolved);
    let again = an_alert("sonarr", 2, Moment::Onset);
    let _ = alerted(&dashboard, &[]).await;

    let first = alerted(&dashboard, std::slice::from_ref(&onset)).await;
    let unchanged = alerted(&dashboard, std::slice::from_ref(&onset)).await;
    let second = alerted(&dashboard, &[resolved.clone(), onset.clone()]).await;
    let third = alerted(&dashboard, &[again, resolved, onset]).await;

    assert_eq!(named(&first), ["sonarr#1 onset"]);
    assert!(unchanged.is_empty(), "an alert already said was said again");
    assert_eq!(named(&second), ["sonarr#1 resolved"]);
    assert_eq!(named(&third), ["sonarr#2 onset"]);
}

/// Several new at once are said oldest first, which is the order they happened in; the
/// list carries them newest first.
#[tokio::test]
async fn several_new_at_once_are_said_in_the_order_they_happened() {
    let dashboard = a_dashboard();
    let _ = alerted(&dashboard, &[]).await;
    let said = alerted(
        &dashboard,
        &[
            an_alert("radarr", 1, Moment::Onset),
            an_alert("sonarr", 1, Moment::Onset),
        ],
    )
    .await;
    assert_eq!(named(&said), ["sonarr#1 onset", "radarr#1 onset"]);
}

/// An alert recorded before alerts carried an identity has nothing to be told apart by,
/// so it is never said as one happening.
#[tokio::test]
async fn an_alert_with_no_identity_is_never_said_as_happening() {
    let dashboard = a_dashboard();
    let _ = alerted(&dashboard, &[]).await;
    let unnamed = Alert {
        id: None,
        ..an_alert("sonarr", 1, Moment::Onset)
    };
    assert!(alerted(&dashboard, &[unnamed]).await.is_empty());
}

/// What is playing is said to a listener that has just arrived, and is not read again
/// before its pace comes round.
#[tokio::test]
async fn what_is_playing_is_said_on_arrival_and_at_its_pace() {
    let dashboard = a_dashboard();
    let first = playing(&dashboard, true).await;
    assert_eq!(first.map(|said| said.kind()), Some(kind::PLAYING));
    assert_eq!(playing(&dashboard, false).await, None);
    assert!(
        playing(&dashboard, true).await.is_some(),
        "a new listener is told again"
    );
}

/// Read again at its pace and unchanged, it is not said again.
#[tokio::test]
async fn what_is_playing_unchanged_is_not_said_again() {
    let dashboard = a_dashboard();
    let _ = playing(&dashboard, true).await;
    dashboard.playing.lock().await.0 = None;
    assert_eq!(playing(&dashboard, false).await, None);
}
