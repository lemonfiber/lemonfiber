use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::{Http, Request, Response};

use super::{by_digest, json_by_digest};

/// What a registry names `body` by.
pub(crate) fn named(body: &str) -> String {
    format!("sha256:{}", lemonfiber_ports::digest::sha256(body))
}

/// A registry answering each request by the first of `routes` whose fragment its URL
/// carries, and 404 to the rest.
pub(crate) fn serving(routes: Vec<(String, Response)>) -> Arc<Fake> {
    Fake::handing(move |request: Request| {
        let answered = routes
            .iter()
            .find(|(fragment, _)| request.url.contains(fragment.as_str()))
            .map_or_else(
                || Response {
                    status: 404,
                    headers: Vec::new(),
                    body: String::new(),
                },
                |(_, response)| response.clone(),
            );
        Box::pin(std::future::ready(Ok(answered)))
    })
}

/// An answer of `status` with `body`, and the headers `headers`.
pub(crate) fn answered(status: u16, headers: &[(&str, &str)], body: &str) -> Response {
    Response {
        status,
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        body: body.to_owned(),
    }
}

const BLOB: &str = "https://ghcr.io/v2/lemonfiber/caddy/blobs/";

async fn read(reaching: &dyn Http, digest: &str) -> Result<String, String> {
    by_digest(
        reaching,
        ("ghcr.io", &format!("{BLOB}{digest}"), "*/*"),
        digest,
        &mut Some("anonymous".to_owned()),
    )
    .await
}

#[tokio::test]
async fn what_a_registry_serves_by_digest_is_read_where_it_is_what_the_digest_names() {
    let body = r#"{"served":true}"#;
    let digest = named(body);
    let asked = serving(vec![(digest.clone(), answered(200, &[], body))]);
    assert_eq!(read(asked.as_ref(), &digest).await, Ok(body.to_owned()));
}

#[tokio::test]
async fn what_a_registry_serves_under_another_digest_is_refused() {
    let digest = named("asked for");
    let asked = serving(vec![(digest.clone(), answered(200, &[], "served instead"))]);
    let why = read(asked.as_ref(), &digest).await;
    assert!(
        why.as_ref()
            .is_err_and(|why| why.contains(&named("served instead")) && why.contains(&digest)),
        "{why:?}"
    );
}

#[tokio::test]
async fn a_read_handed_on_is_followed_once_to_its_https_address_without_the_token() {
    let body = "the blob";
    let digest = named(body);
    let asked = serving(vec![
        ("https://cdn.example/".to_owned(), answered(200, &[], body)),
        (
            digest.clone(),
            answered(307, &[("location", "https://cdn.example/signed")], ""),
        ),
    ]);
    assert_eq!(read(asked.as_ref(), &digest).await, Ok(body.to_owned()));
    let handed: Vec<Request> = asked
        .requests()
        .into_iter()
        .filter(|request| request.url.starts_with("https://cdn.example/"))
        .collect();
    assert_eq!(handed.len(), 1, "{handed:?}");
    assert!(
        handed.iter().all(|request| request
            .headers
            .iter()
            .all(|(name, _)| name != "Authorization")),
        "the registry's token went with the read it handed on"
    );
}

#[tokio::test]
async fn a_read_handed_on_to_no_https_address_or_refused_there_is_refused() {
    let digest = named("the blob");
    for location in [None, Some("http://cdn.example/signed")] {
        let headers: Vec<(&str, &str)> = location.map(|to| ("location", to)).into_iter().collect();
        let asked = serving(vec![(digest.clone(), answered(307, &headers, ""))]);
        let why = read(asked.as_ref(), &digest).await;
        assert!(
            why.as_ref()
                .is_err_and(|why| why.contains("no https address")),
            "{why:?}"
        );
    }
    let refused = serving(vec![
        ("https://cdn.example/".to_owned(), answered(403, &[], "")),
        (
            digest.clone(),
            answered(307, &[("location", "https://cdn.example/signed")], ""),
        ),
    ]);
    let why = read(refused.as_ref(), &digest).await;
    assert!(
        why.as_ref().is_err_and(|why| why.contains("answered 403")),
        "{why:?}"
    );
    let silent = Fake::always(Answer::Silent);
    assert!(read(silent.as_ref(), &digest).await.is_err());
}

#[tokio::test]
async fn what_is_served_by_digest_and_is_no_json_is_refused_as_json() {
    let digest = named("not json");
    let asked = serving(vec![(digest.clone(), answered(200, &[], "not json"))]);
    let read = json_by_digest(
        asked.as_ref(),
        ("ghcr.io", &format!("{BLOB}{digest}"), "*/*"),
        &digest,
        &mut None,
    )
    .await;
    assert!(read.is_err_and(|why| why.contains("no JSON")));
}
