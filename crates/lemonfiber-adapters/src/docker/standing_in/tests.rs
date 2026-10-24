use std::sync::Arc;

use lemonfiber_ports::serving::{Said, StandIn};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

use super::{answer, answering, filled, response, Heard};
use crate::AFTER_A_FAILED_ACCEPT;

const VERSION: &str = r#"{"version":"4.5.1"}"#;

fn a_stand_in() -> StandIn {
    StandIn {
        port: 8080,
        headers: vec![("X-Application-Version".to_owned(), "4.5.1".to_owned())],
        answers: vec![
            Said {
                query: Some(("mode".to_owned(), "version".to_owned())),
                ..Said::json("GET", "/api", VERSION.to_owned())
            },
            Said::empty("POST", "/Auth/Keys"),
        ],
    }
}

#[test]
fn what_was_given_is_answered_by_its_method_path_and_parameter_and_nothing_else_is() {
    let stand_in = a_stand_in();

    assert_eq!(
        answer(
            &stand_in,
            "GET",
            "/api",
            "apikey=k&mode=version&output=json"
        ),
        (200, Some(VERSION))
    );
    assert_eq!(
        answer(&stand_in, "GET", "/API", "mode=version").1,
        Some(VERSION)
    );
    assert_eq!(
        answer(&stand_in, "GET", "/api", "verbose&mode=version").1,
        Some(VERSION)
    );
    assert_eq!(answer(&stand_in, "POST", "/api", "mode=version").0, 404);
    assert_eq!(answer(&stand_in, "GET", "/api", "mode=shutdown").0, 404);
    assert_eq!(answer(&stand_in, "GET", "/api", "").0, 404);
    assert_eq!(answer(&stand_in, "GET", "/api", "modes=version").0, 404);
    assert_eq!(answer(&stand_in, "GET", "/other", "mode=version").0, 404);
    assert_eq!(
        answer(&stand_in, "POST", "/auth/keys", "App=conformance"),
        (204, None)
    );
}

#[test]
fn an_answer_carries_every_header_given_and_a_body_only_where_it_has_one() {
    let stand_in = a_stand_in();

    assert_eq!(
        response(&stand_in, "GET", "/api", "mode=version", &Heard::new()),
        format!(
            "HTTP/1.1 200 \r\nX-Application-Version: 4.5.1\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{VERSION}",
            VERSION.len()
        )
    );
    assert_eq!(
        response(&stand_in, "POST", "/Auth/Keys", "", &Heard::new()),
        "HTTP/1.1 204 \r\nX-Application-Version: 4.5.1\r\nConnection: close\r\n\r\n"
    );
}

#[test]
fn a_body_takes_what_a_request_said_earlier_written_as_json_and_nothing_where_none_did() {
    let heard = Heard::from([
        ("App".to_owned(), "conformance".to_owned()),
        ("odd".to_owned(), "a\"b".to_owned()),
    ]);

    assert_eq!(
        filled(
            r#"{"AppName":"{query:App}","Other":"{query:Unheard}"}"#,
            &heard
        ),
        r#"{"AppName":"conformance","Other":""}"#
    );
    assert_eq!(filled(r#"["{query:odd}"]"#, &heard), r#"["a\"b"]"#);
    assert_eq!(filled(r#"["{query:App"]"#, &heard), r#"["{query:App"]"#);
}

#[tokio::test(start_paused = true)]
async fn a_connection_that_could_not_be_taken_is_waited_past_and_the_next_one_answered() {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok();
    let at = listener.as_ref().and_then(|held| held.local_addr().ok());
    let (client, taken) = tokio::join!(async { TcpStream::connect(at?).await.ok() }, async {
        listener?.accept().await.ok()
    });
    let failed = Err(std::io::Error::other("no descriptors left"));
    let next = taken
        .map(|(stream, _)| stream)
        .ok_or_else(|| std::io::Error::other("nothing was taken"));
    let started = tokio::time::Instant::now();

    answering(
        Box::pin(tokio_stream::iter([failed, next])),
        Arc::new(a_stand_in()),
    )
    .await;
    let waited = started.elapsed();
    let mut said = String::new();
    if let Some(mut client) = client {
        let _ = client
            .write_all(b"GET /api?mode=version HTTP/1.1\r\nHost: conformance-download\r\n\r\n")
            .await;
        let _ = client.read_to_string(&mut said).await;
    }

    assert_eq!(waited, AFTER_A_FAILED_ACCEPT);
    assert!(said.starts_with("HTTP/1.1 200 \r\n"), "{said}");
    assert!(said.ends_with(r#"{"version":"4.5.1"}"#), "{said}");
}
