use std::sync::Arc;

use lemonfiber_ports::serving::StandIn;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

use super::{answer, answering};
use crate::AFTER_A_FAILED_ACCEPT;

fn a_stand_in() -> StandIn {
    StandIn {
        port: 8080,
        asked_by: "mode".to_owned(),
        answers: vec![("version".to_owned(), r#"{"version":"4.5.1"}"#.to_owned())],
    }
}

#[test]
fn what_was_given_is_answered_by_the_value_of_its_parameter_and_nothing_else_is() {
    let stand_in = a_stand_in();

    assert_eq!(
        answer(&stand_in, "/api?apikey=k&mode=version&output=json"),
        ("200 OK", r#"{"version":"4.5.1"}"#)
    );
    assert_eq!(answer(&stand_in, "/api?mode=shutdown").0, "404 Not Found");
    assert_eq!(answer(&stand_in, "/api").0, "404 Not Found");
    assert_eq!(answer(&stand_in, "/api?modes=version").0, "404 Not Found");
    assert_eq!(
        answer(&stand_in, "/api?verbose&mode=version").1,
        r#"{"version":"4.5.1"}"#
    );
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
    assert!(said.starts_with("HTTP/1.1 200 OK\r\n"), "{said}");
    assert!(said.ends_with(r#"{"version":"4.5.1"}"#), "{said}");
}
