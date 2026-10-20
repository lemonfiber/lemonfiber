use std::cell::Cell;

use super::{acts, passed, Asking, CameTo, Client, Last, Pass};
use crate::journal::{Action, Kind};

const LAST: Last = Last { port: Some(51413) };

fn client(service: &str, came_to: CameTo) -> Client {
    Client {
        service: service.to_owned(),
        came_to,
    }
}

#[test]
fn an_answer_that_differs_from_the_last_is_acted_on() {
    assert!(acts(Asking::Changed, Some(51999), LAST));
    assert!(acts(Asking::Changed, Some(51413), Last::default()));
}

#[test]
fn an_answer_the_same_as_the_last_is_left_on_the_cadence_and_compared_afresh() {
    assert!(!acts(Asking::Changed, Some(51413), LAST));
    assert!(acts(Asking::Afresh, Some(51413), LAST));
}

#[test]
fn forwarding_that_ended_is_acted_on_and_none_ever_forwarded_is_not() {
    assert!(acts(Asking::Changed, None, LAST));
    assert!(!acts(Asking::Changed, None, Last::default()));
    assert!(!acts(Asking::Afresh, None, Last::default()));
}

#[tokio::test]
async fn a_client_already_on_the_answer_is_not_written() {
    let wrote = Cell::new(false);
    let came_to = passed(Some(51413), Ok(Some(51413)), |_| {
        wrote.set(true);
        async { Ok(()) }
    })
    .await;
    assert_eq!(came_to, CameTo::Held);
    assert!(!wrote.get());
}

#[tokio::test]
async fn a_client_on_another_port_or_unread_is_set_to_the_answer() {
    let asked = Cell::new(None);
    let came_to = passed(Some(51999), Ok(Some(51413)), |port| {
        asked.set(Some(port));
        async { Ok(()) }
    })
    .await;
    assert_eq!(
        came_to,
        CameTo::Moved {
            from: Some(Some(51413))
        }
    );
    assert_eq!(asked.get(), Some(Some(51999)));
    let unread = passed(Some(51999), Err("silent".to_owned()), |_| async { Ok(()) }).await;
    assert_eq!(unread, CameTo::Moved { from: None });
}

#[tokio::test]
async fn forwarding_that_ended_sets_the_client_to_none() {
    let asked = Cell::new(Some(Some(1)));
    let came_to = passed(None, Ok(Some(51413)), |port| {
        asked.set(Some(port));
        async { Ok(()) }
    })
    .await;
    assert_eq!(asked.get(), Some(None));
    let pass = Pass {
        answer: None,
        clients: vec![client("qbittorrent", came_to)],
        unrecorded: None,
    };
    assert_eq!(
        pass.said(),
        vec!["forwarding ended, so qbittorrent was set back to no forwarded port".to_owned()]
    );
}

#[tokio::test]
async fn a_set_refused_is_said_in_the_refusers_words_and_raised() {
    let came_to = passed(Some(51999), Ok(Some(51413)), |_| async {
        Err("qBittorrent answered unexpectedly".to_owned())
    })
    .await;
    let pass = Pass {
        answer: Some(51999),
        clients: vec![client("qbittorrent", came_to)],
        unrecorded: None,
    };
    assert!(!pass.passed());
    assert!(pass.changes("1000").is_empty());
    assert_eq!(
        pass.said(),
        vec![
            "the forwarded port 51999 was not passed to qbittorrent: qBittorrent answered \
             unexpectedly"
                .to_owned()
        ]
    );
    let fault = pass.fault();
    assert!(fault.as_ref().is_some_and(
        |fault| fault.summary.contains("51999") && fault.summary.contains("qbittorrent")
    ));
}

#[test]
fn a_moved_port_is_journalled_as_the_clients_own_field_and_put_back_through_it() {
    let pass = Pass {
        answer: Some(51999),
        clients: vec![
            client(
                "qbittorrent",
                CameTo::Moved {
                    from: Some(Some(51413)),
                },
            ),
            client("deluge", CameTo::Held),
            client("transmission", CameTo::Moved { from: Some(None) }),
        ],
        unrecorded: Some("not recorded".to_owned()),
    };
    assert!(pass.passed());
    assert_eq!(pass.fault(), None);
    let changes = pass.changes("1000");
    let kinds: Vec<(String, Kind)> = changes
        .iter()
        .map(|change| (change.target.clone(), change.kind.clone()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (
                "qbittorrent".to_owned(),
                Kind::Configured {
                    resource: "peer port".to_owned(),
                    id: "qbittorrent".to_owned(),
                    field: "peer_port".to_owned(),
                    previous: Some("51413".to_owned()),
                    current: "51999".to_owned(),
                }
            ),
            (
                "transmission".to_owned(),
                Kind::Configured {
                    resource: "peer port".to_owned(),
                    id: "transmission".to_owned(),
                    field: "peer_port".to_owned(),
                    previous: Some("none".to_owned()),
                    current: "51999".to_owned(),
                }
            ),
        ]
    );
    assert!(changes
        .iter()
        .all(|change| change.operation == "vpn port forwarding"
            && matches!(change.undo().action, Action::Reconfigure { .. })));
    assert_eq!(pass.said().len(), 3);
    assert_eq!(pass.said().last().map(String::as_str), Some("not recorded"));
}
