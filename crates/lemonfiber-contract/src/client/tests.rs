use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::{Fetched, Http, Method, Request, Response, Unreachable};
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
        .call("download.torrent", 1, "moved", LARGEST, &"2026-10")
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

#[tokio::test]
async fn a_request_that_cannot_be_written_is_refused_and_never_sent() {
    let (fake, client, _) = adapter(Answer::reply(200, "7"));
    // JSON keys are strings, so a map keyed by pairs has no JSON to be written as.
    let unwritable = std::collections::BTreeMap::from([((1_u8, 2_u8), 3_u8)]);
    assert!(matches!(
        client
            .call::<_, u32>("download.torrent", 1, "moved", LARGEST, &unwritable)
            .await,
        Err(Failure::Refused { .. })
    ));
    assert!(fake.request().is_none());
}

/// An adapter answering bytes that are not text.
struct Binary;

#[async_trait]
impl Http for Binary {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        Err(Unreachable::once(&request.url, "only fetched"))
    }

    async fn fetch(&self, _request: &Request, _most: usize) -> Result<Fetched, Unreachable> {
        Ok(Fetched {
            status: 200,
            headers: Vec::new(),
            bytes: Some(vec![0xff, 0xfe]),
        })
    }
}

#[tokio::test]
async fn an_answer_that_is_not_text_is_refused_and_witnessed() {
    let told = Arc::new(Told::default());
    let client = Contracted::new(
        Arc::new(Binary),
        "http://adapter:8080",
        "plex-adapter",
        "k3y",
    )
    .witnessed_by(told.clone());
    assert!(matches!(
        asked::<u32>(&client).await,
        Err(Failure::Refused { .. })
    ));
    assert_eq!(told.count(), 1);
}

#[tokio::test]
async fn an_answer_is_read_up_to_the_bound_its_operation_declares() {
    let (_, client, told) = adapter(Answer::reply(200, format!("\"{}\"", "a".repeat(LARGEST))));
    let read: Result<String, Failure> = client
        .call("media.serve", 1, "picture", LARGEST * 2, &())
        .await;
    assert_eq!(read.ok().map(|answer| answer.len()), Some(LARGEST));
    assert_eq!(told.count(), 0);
}

#[tokio::test]
async fn an_adapter_is_asked_what_it_is_by_a_keyed_get() {
    let about = r#"{"speaks":["media.serve@1"],"upstream":"a media server","releases":[]}"#;
    let (fake, client, told) = adapter(Answer::reply(200, about));
    let said = client.about().await;
    assert_eq!(
        said.ok().map(|about| about.speaks),
        Some(vec!["media.serve@1".to_owned()])
    );
    let request = fake.request();
    assert_eq!(
        request
            .as_ref()
            .map(|request| (request.method, request.url.as_str())),
        Some((
            Method::Get,
            "http://adapter:8080/lemonfiber/adapter/v1/about"
        ))
    );
    assert_eq!(request.and_then(|request| request.body), None);
    assert_eq!(told.count(), 0);

    let (_, client, told) = adapter(Answer::reply(200, r#"{"speaks":[]}"#));
    assert!(matches!(client.about().await, Err(Failure::Refused { .. })));
    assert_eq!(told.count(), 1);
}

#[tokio::test]
async fn a_call_without_the_key_carries_none_and_answers_its_status() {
    let (fake, client, told) = adapter(Answer::reply(401, ""));
    let serve = crate::capabilities::media::serve::capability();
    let first = serve.operations.first();
    let status = match first {
        Some(operation) => client.unkeyed(operation).await.ok(),
        None => None,
    };
    assert_eq!(status, Some(401));
    let request = fake.request();
    assert!(request.as_ref().is_some_and(|request| !request
        .headers
        .iter()
        .any(|(name, _)| name == "Authorization")));
    assert_eq!(
        request.and_then(|request| request.body).as_deref(),
        Some("{}")
    );
    assert_eq!(told.count(), 0);

    let silent = Contracted::new(Fake::silent(), "http://adapter:8080", "plex-adapter", "k3y");
    match first {
        Some(operation) => assert!(matches!(
            silent.unkeyed(operation).await,
            Err(Failure::Unavailable { .. })
        )),
        None => unreachable!("media.serve carries operations"),
    }
}
