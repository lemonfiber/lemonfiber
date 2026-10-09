use async_trait::async_trait;

use super::{Fetched, Http, Method, Request, Response, Unreachable};

/// An answer with nothing said about how it was served.
fn answered(status: u16) -> Response {
    Response {
        status,
        headers: Vec::new(),
        body: String::new(),
    }
}

#[test]
fn a_two_hundred_is_a_success_and_a_refusal_is_not() {
    assert!(answered(200).is_success());
    assert!(answered(204).is_success());
    for status in [199, 300, 401, 500] {
        assert!(!answered(status).is_success());
    }
}

/// Whatever case the service spelled the name in, and the first where it repeated.
#[test]
fn a_header_is_found_however_the_service_spelled_its_name() {
    let response = Response {
        status: 200,
        headers: vec![
            ("Content-Type".to_owned(), "application/json".to_owned()),
            ("content-type".to_owned(), "text/xml".to_owned()),
            ("X-Served-By".to_owned(), "plex".to_owned()),
        ],
        body: String::new(),
    };
    assert_eq!(response.header("content-type"), Some("application/json"));
    assert_eq!(response.header("CONTENT-TYPE"), Some("application/json"));
    assert_eq!(response.header("x-served-by"), Some("plex"));
}

/// And a header nothing sent is absent rather than empty.
///
/// The half that decides whether the reading means anything: a lookup answering
/// `Some("")` for a header that never arrived would let a caller conclude the
/// service said something it did not.
#[test]
fn a_header_the_service_did_not_send_is_absent() {
    assert_eq!(answered(200).header("content-type"), None);
    let response = Response {
        status: 200,
        headers: vec![("Content-Length".to_owned(), "0".to_owned())],
        body: String::new(),
    };
    assert_eq!(response.header("content-type"), None);
}

#[test]
fn unreachable_names_the_url_and_the_reason() {
    let failure = Unreachable {
        url: "http://sonarr:8989/api".to_owned(),
        reason: "connection refused".to_owned(),
        attempts: 1,
        connected: false,
    };
    let rendered = failure.to_string();
    assert!(rendered.contains("sonarr"));
    assert!(rendered.contains("connection refused"));
}

#[test]
fn a_request_is_plain_data() {
    let request = Request {
        method: Method::Post,
        url: "http://sonarr:8989/api/v3/rootfolder".to_owned(),
        headers: vec![("X-Api-Key".to_owned(), "secret".to_owned())],
        body: Some("{}".to_owned()),
        pinned: None,
    };
    assert_eq!(request.clone(), request);
    assert_eq!(request.method, Method::Post);
}

/// A transport that answers every request with a short JPEG-typed body.
struct Answering;

#[async_trait]
impl Http for Answering {
    async fn send(&self, _request: &Request) -> Result<Response, Unreachable> {
        Ok(Response {
            status: 200,
            headers: vec![("Content-Type".to_owned(), "image/jpeg".to_owned())],
            body: "four".to_owned(),
        })
    }
}

fn get() -> Request {
    Request {
        method: Method::Get,
        url: "http://media:8096/Items/f1/Images/Primary".to_owned(),
        headers: Vec::new(),
        body: None,
        pinned: None,
    }
}

#[tokio::test]
async fn a_fetch_holds_what_fits_and_nothing_of_what_does_not() {
    let fetched = Answering.fetch(&get(), 4).await;
    assert_eq!(
        fetched,
        Ok(Fetched {
            status: 200,
            headers: vec![("Content-Type".to_owned(), "image/jpeg".to_owned())],
            bytes: Some(b"four".to_vec()),
        })
    );
    let over = Answering.fetch(&get(), 3).await;
    assert_eq!(over.map(|fetched| fetched.bytes), Ok(None));
}

#[tokio::test]
async fn a_fetched_answer_says_its_success_and_headers_as_a_response_does() {
    let shared: std::sync::Arc<dyn Http> = std::sync::Arc::new(Answering);
    let fetched = shared.fetch(&get(), 4).await.ok();
    assert_eq!(
        fetched
            .as_ref()
            .map(|fetched| (fetched.is_success(), fetched.header("content-type"))),
        Some((true, Some("image/jpeg")))
    );
    let refused = Fetched {
        status: 404,
        headers: Vec::new(),
        bytes: None,
    };
    assert!(!refused.is_success() && refused.header("content-type").is_none());
}
