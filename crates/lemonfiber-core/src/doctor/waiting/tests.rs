use std::sync::Arc;

use async_trait::async_trait;

use super::super::{Check, Verdict};
use super::{Holds, WaitingCheck};
use crate::model::{Waiting, Wanted};

/// Services held back as given, or a record that cannot be read.
struct Held(Result<Vec<Waiting>, ()>);

#[async_trait]
impl Holds for Held {
    async fn waiting(&self) -> Result<Vec<Waiting>, ()> {
        self.0.clone()
    }
}

async fn found(waiting: Result<Vec<Waiting>, ()>) -> Vec<Verdict> {
    WaitingCheck::new(Arc::new(Held(waiting)))
        .run()
        .await
        .into_iter()
        .map(|finding| finding.verdict)
        .collect()
}

fn held(service: &str, wants: Vec<Wanted>) -> Waiting {
    Waiting {
        service: service.to_owned(),
        wants,
    }
}

#[tokio::test]
async fn each_service_held_back_is_warned_of_with_what_it_waits_for() {
    let one = found(Ok(vec![held(
        "tunnel",
        vec![Wanted::Secret {
            secret: "guard:wireguard-key".to_owned(),
        }],
    )]))
    .await;
    assert!(
        matches!(one.as_slice(), [Verdict::Warn(problem)]
            if problem.summary == "tunnel is held back until you give what it needs to start"
                && problem.meaning.starts_with("tunnel waits for the secret guard:wireguard-key.")),
        "{one:?}"
    );
    let two = found(Ok(vec![
        held(
            "tunnel",
            vec![Wanted::Setting {
                setting: "guard:account".to_owned(),
                choices: None,
            }],
        ),
        held(
            "torrents",
            vec![Wanted::Service {
                service: "tunnel".to_owned(),
            }],
        ),
    ]))
    .await;
    assert!(
        matches!(two.as_slice(), [Verdict::Warn(problem)]
            if problem.summary
                == "tunnel, torrents are held back until you give what they need to start"),
        "{two:?}"
    );
}

#[tokio::test]
async fn nothing_waiting_passes_and_an_unread_record_is_unverified() {
    assert!(matches!(
        found(Ok(Vec::new())).await.as_slice(),
        [Verdict::Pass { note: None }]
    ));
    assert!(matches!(
        found(Err(())).await.as_slice(),
        [Verdict::Unverified { .. }]
    ));
}
