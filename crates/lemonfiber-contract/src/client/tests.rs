use std::sync::{Arc, Mutex};

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::Failure;

use super::{Contracted, Witness, LARGEST};

/// Records what it was told.
#[derive(Default)]
struct Told(Mutex<Vec<String>>);

impl Witness for Told {
    fn nonconforming(&self, operation: &str, why: &str) {
        if let Ok(mut told) = self.0.lock() {
            told.push(format!("{operation}: {why}"));
        }
    }
}

impl Told {
    fn count(&self) -> usize {
        self.0.lock().map(|told| told.len()).unwrap_or_default()
    }
}

/// An adapter answering `answer`, and what it was told about answers outside the contract.
fn adapter(answer: Answer) -> (Arc<Fake>, Contracted, Arc<Told>) {
    let fake = Fake::always(answer);
    let told = Arc::new(Told::default());
    let client = Contracted::new(fake.clone(), "http://adapter:8080/", "plex-adapter", "k3y")
        .witnessed_by(told.clone());
    (fake, client, told)
}

async fn asked<R: serde::de::DeserializeOwned>(client: &Contracted) -> Result<R, Failure> {
    client
        .call("download.torrent", 1, "moved", &"2026-10")
        .await
}

#[tokio::test]
async fn a_call_is_a_keyed_post_to_its_path_and_its_answer_is_read() {
    let (fake, client, told) = adapter(Answer::reply(200, "7"));
    assert_eq!(asked::<u32>(&client).await.ok(), Some(7));
    let request = fake.request();
    assert_eq!(
        request.as_ref().map(|request| request.method),
        Some(Method::Post)
    );
    assert_eq!(
        request.as_ref().map(|request| request.url.as_str()),
        Some("http://adapter:8080/lemonfiber/download.torrent/v1/moved")
    );
    assert!(request.as_ref().is_some_and(|request| request
        .headers
        .contains(&("Authorization".to_owned(), "Bearer k3y".to_owned()))));
    assert_eq!(
        request.and_then(|request| request.body).as_deref(),
        Some("\"2026-10\"")
    );
    assert_eq!(told.count(), 0);
}

#[tokio::test]
async fn only_an_answer_with_nothing_to_say_may_be_empty() {
    let (_, client, told) = adapter(Answer::reply(204, ""));
    assert!(asked::<()>(&client).await.is_ok());
    let (_, client, told_empty) = adapter(Answer::reply(200, ""));
    assert!(matches!(
        asked::<Option<u32>>(&client).await,
        Err(Failure::Refused { .. })
    ));
    assert_eq!((told.count(), told_empty.count()), (0, 1));
}

#[tokio::test]
async fn an_answer_outside_the_contract_is_refused_and_witnessed() {
    let outside = [
        Answer::reply(200, "\"seven\""),
        Answer::reply(418, "7"),
        Answer::reply(200, " ".repeat(LARGEST + 1)),
        Answer::served(
            503,
            "application/problem+json",
            r#"{"type":"refused","detail":"x"}"#,
        ),
        Answer::served(500, "application/problem+json", r#"{"nonsense":true}"#),
    ];
    for answer in outside {
        let (_, client, told) = adapter(answer);
        assert!(matches!(
            asked::<u32>(&client).await,
            Err(Failure::Refused { .. })
        ));
        assert_eq!(told.count(), 1);
    }
}

#[tokio::test]
async fn a_declared_refusal_is_read_as_the_failure_it_names() {
    let (_, client, told) = adapter(Answer::served(
        503,
        "application/problem+json; charset=utf-8",
        r#"{"type":"unavailable","detail":"plex is down"}"#,
    ));
    assert!(matches!(
        asked::<u32>(&client).await,
        Err(Failure::Unavailable { service }) if service == "plex-adapter"
    ));
    assert_eq!(told.count(), 0);
}

#[tokio::test]
async fn a_refused_key_and_an_absent_adapter_are_told_apart() {
    let (_, client, _) = adapter(Answer::reply(401, ""));
    assert!(matches!(
        asked::<u32>(&client).await,
        Err(Failure::Unauthorised { .. })
    ));
    let client = Contracted::new(Fake::silent(), "http://adapter:8080", "plex-adapter", "k3y");
    assert!(matches!(
        asked::<u32>(&client).await,
        Err(Failure::Unavailable { .. })
    ));
}

#[test]
fn the_key_is_never_in_what_is_debugged() {
    let client = Contracted::new(Fake::silent(), "http://adapter:8080", "plex-adapter", "k3y");
    let debugged = format!("{client:?}");
    assert!(!debugged.contains("k3y"));
    assert_eq!(client.service(), "plex-adapter");
}

/// A client nobody keeps standing for still refuses what is outside the contract.
#[tokio::test]
async fn an_unwitnessed_client_still_refuses_an_answer_outside_the_contract() {
    let client = Contracted::new(
        Fake::always(Answer::reply(204, "7")),
        "http://adapter:8080",
        "plex-adapter",
        "k3y",
    );
    assert!(matches!(
        asked::<()>(&client).await,
        Err(Failure::Refused { .. })
    ));
}
