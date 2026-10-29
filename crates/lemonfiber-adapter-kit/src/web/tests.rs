use lemonfiber_ports::http::{Http, Method, Request};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::{client_builder, method, query_of, read_as_text, with_tls, withheld, Web};

async fn answering(answer: &'static str) -> Option<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
    let address = listener.local_addr().ok()?;
    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut asked = [0_u8; 4096];
            let _ = stream.read(&mut asked).await;
            let _ = stream.write_all(answer.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    Some(format!("http://{address}/"))
}

fn asking(url: String) -> Request {
    Request {
        method: Method::Get,
        url,
        headers: vec![("Accept".to_owned(), "application/json".to_owned())],
        body: None,
        pinned: None,
    }
}

#[tokio::test]
async fn an_answer_is_read_with_its_status_headers_and_body() {
    let url = answering(
        "HTTP/1.1 404 Not Found\r\ncontent-type: application/json\r\ncontent-length: 2\r\n\r\n{}",
    )
    .await;
    let read = match (url, Web::new()) {
        (Some(url), Ok(web)) => web.send(&asking(url)).await.ok(),
        _ => None,
    };
    let read = read.map(|response| {
        (
            response.status,
            response.header("Content-Type").map(str::to_owned),
            response.body,
        )
    });
    assert_eq!(
        read,
        Some((404, Some("application/json".to_owned()), "{}".to_owned()))
    );
}

#[tokio::test]
async fn a_redirect_is_answered_rather_than_followed() {
    let url = answering(
        "HTTP/1.1 302 Found\r\nlocation: http://192.0.2.1/elsewhere\r\ncontent-length: 0\r\n\r\n",
    )
    .await;
    let status = match (url, Web::new()) {
        (Some(url), Ok(web)) => web.send(&asking(url)).await.ok().map(|read| read.status),
        _ => None,
    };
    assert_eq!(status, Some(302));
}

#[tokio::test]
async fn a_body_past_what_was_asked_for_is_held_as_no_body() {
    let url = answering("HTTP/1.1 200 OK\r\ncontent-length: 10\r\n\r\n0123456789").await;
    let fetched = match (url, Web::new()) {
        (Some(url), Ok(web)) => web.fetch(&asking(url), 4).await.ok(),
        _ => None,
    };
    assert_eq!(
        fetched.map(|fetched| (fetched.status, fetched.bytes)),
        Some((200, None))
    );
}

#[tokio::test]
async fn nothing_listening_is_unreachable() {
    let address = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener.local_addr().ok(),
        Err(_) => None,
    };
    let reached = match (address, Web::new()) {
        (Some(address), Ok(web)) => Some(
            web.send(&asking(format!("http://{address}/")))
                .await
                .is_err(),
        ),
        _ => None,
    };
    assert_eq!(reached, Some(true));
}

async fn answering_after(wait: std::time::Duration) -> Option<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
    let address = listener.local_addr().ok()?;
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut asked = [0_u8; 4096];
                let _ = stream.read(&mut asked).await;
                tokio::time::sleep(wait).await;
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}")
                    .await;
                let _ = stream.shutdown().await;
            });
        }
    });
    Some(format!("http://{address}/"))
}

#[tokio::test]
async fn a_waiting_client_reads_an_answer_within_its_wait_and_none_after_it() {
    let url = answering_after(std::time::Duration::from_millis(300)).await;
    let (patient, hasty) = match (
        url,
        Web::waiting(std::time::Duration::from_secs(5)),
        Web::waiting(std::time::Duration::from_millis(50)),
    ) {
        (Some(url), Ok(patient), Ok(hasty)) => (
            patient
                .send(&asking(url.clone()))
                .await
                .ok()
                .map(|read| read.status),
            hasty.send(&asking(url)).await.is_err(),
        ),
        _ => (None, false),
    };
    assert_eq!(patient, Some(200));
    assert!(hasty);
}

const BODY: &str = r#"{"Name":"admin"}"#;

#[tokio::test]
async fn a_body_is_sent_as_it_was_written() {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0").await else {
        return;
    };
    let address = listener.local_addr().ok();
    let heard = tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return String::new();
        };
        let mut heard = Vec::new();
        let mut chunk = [0_u8; 1024];
        while !heard.ends_with(BODY.as_bytes()) {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(read) => heard.extend_from_slice(chunk.get(..read).unwrap_or_default()),
            }
        }
        let _ = stream
            .write_all(b"HTTP/1.1 204 No Content\r\ncontent-length: 0\r\n\r\n")
            .await;
        String::from_utf8_lossy(&heard).into_owned()
    });
    let sent = match (address, Web::new()) {
        (Some(address), Ok(web)) => web
            .send(&Request {
                method: Method::Post,
                url: format!("http://{address}/Startup/User"),
                headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
                body: Some(BODY.to_owned()),
                pinned: None,
            })
            .await
            .ok()
            .map(|answer| answer.status),
        _ => None,
    };
    let heard = heard.await.unwrap_or_default();
    assert_eq!(sent, Some(204));
    assert!(heard.starts_with("POST /Startup/User"), "{heard}");
    assert!(heard.ends_with(BODY), "{heard}");
}

#[test]
fn every_method_is_sent_as_itself() {
    assert_eq!(
        [
            Method::Get,
            Method::Post,
            Method::Put,
            Method::Patch,
            Method::Delete
        ]
        .map(method),
        [
            reqwest::Method::GET,
            reqwest::Method::POST,
            reqwest::Method::PUT,
            reqwest::Method::PATCH,
            reqwest::Method::DELETE,
        ]
    );
}

#[tokio::test]
async fn a_text_answer_past_the_bound_is_no_answer_at_all() {
    let url = answering("HTTP/1.1 200 OK\r\ncontent-length: 10\r\n\r\n0123456789").await;
    let read = match (url, Web::new()) {
        (Some(url), Ok(web)) => Some(read_as_text(&web.client, &asking(url), 4).await.is_err()),
        _ => None,
    };
    assert_eq!(read, Some(true));
}

async fn signing_in_then_asking_again(web: Web) -> Option<(String, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
    let address = listener.local_addr().ok()?;
    tokio::spawn(async move {
        for signing_in in [true, false] {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut asked = [0_u8; 4096];
                let read = stream.read(&mut asked).await.unwrap_or_default();
                let carried = String::from_utf8_lossy(asked.get(..read).unwrap_or_default())
                    .to_ascii_lowercase()
                    .contains("cookie: sid=a-session");
                let reply = match (signing_in, carried) {
                    (true, _) => SIGNED_IN,
                    (false, true) => CARRIED,
                    (false, false) => NOT_CARRIED,
                };
                let _ = stream.write_all(reply.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        }
    });
    let url = format!("http://{address}/");
    let signed_in = web.send(&asking(url.clone())).await.ok()?.body;
    let after = web.send(&asking(url)).await.ok()?.body;
    Some((signed_in, after))
}

const SIGNED_IN: &str = "HTTP/1.1 200 OK\r\nset-cookie: SID=a-session; path=/\r\ncontent-length: 3\r\nconnection: close\r\n\r\nOk.";

const CARRIED: &str = "HTTP/1.1 200 OK\r\ncontent-length: 7\r\nconnection: close\r\n\r\ncarried";

const NOT_CARRIED: &str =
    "HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";

#[tokio::test]
async fn a_client_keeping_sessions_carries_the_cookie_a_sign_in_sets_and_no_other_does() {
    let kept = match Web::keeping_sessions() {
        Ok(web) => signing_in_then_asking_again(web).await,
        Err(_) => None,
    };
    assert_eq!(kept, Some(("Ok.".to_owned(), "carried".to_owned())));
    let dropped = match Web::new() {
        Ok(web) => signing_in_then_asking_again(web).await,
        Err(_) => None,
    };
    assert_eq!(dropped, Some(("Ok.".to_owned(), String::new())));
}

/// A query these tests stand a key in, as an upstream that takes its key there is asked.
const KEYED: &str = "mode=queue&r=held-in-the-query";

#[tokio::test]
async fn a_failed_call_shows_none_of_its_query_in_its_url_or_its_reason() {
    let Ok(listener) = std::net::TcpListener::bind("127.0.0.1:0") else {
        return;
    };
    let address = listener
        .local_addr()
        .map(|at| at.to_string())
        .unwrap_or_default();
    drop(listener);
    let failed = match Web::new() {
        Ok(web) => web
            .send(&asking(format!("http://{address}/api?{KEYED}")))
            .await
            .err(),
        Err(_) => None,
    };
    let said = format!("{failed:?}");
    assert!(failed.is_some(), "the call reached something");
    assert!(
        !said.contains("0f0f0f0f") && !said.contains("apikey"),
        "{said}"
    );
}

#[test]
fn only_a_query_that_holds_something_is_withheld() {
    assert_eq!(query_of("http://upstream/api"), None);
    assert_eq!(query_of("http://upstream/api?"), None);
    assert_eq!(
        query_of(&format!("http://upstream/api?{KEYED}")),
        Some(KEYED)
    );
    assert_eq!(withheld("plain words", None), "plain words");
    assert!(!withheld(&format!("failed at /api?{KEYED}"), Some(KEYED)).contains(KEYED));
}

#[test]
fn a_client_is_built_over_the_bundled_tls() {
    assert!(client_builder().build().is_ok());
}

#[test]
fn a_client_without_tls_is_refused_rather_than_built_over_another() {
    assert!(with_tls(None).build().is_err());
}

/// The fake's own address, an app answering `200` at its root, and the client's answer to it.
async fn asked_over_tls(client: reqwest::Client) -> Option<Result<u16, reqwest::Error>> {
    let app = axum::Router::new().route("/", axum::routing::get(|| async { "ok" }));
    let served = lemonfiber_pinned::fake::tls(app)?;
    let answer = client.get(served.base.as_url().clone()).send().await;
    Some(answer.map(|answer| answer.status().as_u16()))
}

#[tokio::test]
async fn a_client_over_the_kit_tls_completes_a_handshake_with_a_root_it_trusts() {
    let app = axum::Router::new().route("/", axum::routing::get(|| async { "ok" }));
    let served = lemonfiber_pinned::fake::tls(app);
    let config = served.as_ref().and_then(|served| {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(served.certificate.clone()).ok()?;
        rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .ok()
        .map(|config| config.with_root_certificates(roots).with_no_client_auth())
    });
    let client = with_tls(config).build().ok();
    let answered = match (served, client) {
        (Some(served), Some(client)) => client
            .get(served.base.as_url().clone())
            .send()
            .await
            .ok()
            .map(|answer| answer.status().as_u16()),
        _ => None,
    };
    assert_eq!(answered, Some(200));
}

#[tokio::test]
async fn a_client_from_client_builder_refuses_a_certificate_no_bundled_root_signs() {
    let refused = match client_builder().build() {
        Ok(client) => asked_over_tls(client).await,
        Err(_) => None,
    };
    assert!(refused.is_some_and(|answer| answer.is_err_and(|error| error.is_connect())));
}
