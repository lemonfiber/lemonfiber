use std::sync::Arc;

use lemonfiber_ports::serving::StandIn;
use tokio::net::TcpStream;

use super::{answer, answering, AFTER_A_FAILED_ACCEPT};

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
async fn a_connection_that_could_not_be_taken_is_waited_past_and_the_next_one_taken() {
    let failed =
        || -> std::io::Result<TcpStream> { Err(std::io::Error::other("no descriptors left")) };
    let started = tokio::time::Instant::now();

    answering(
        Box::pin(tokio_stream::iter([failed(), failed()])),
        Arc::new(a_stand_in()),
    )
    .await;

    assert_eq!(started.elapsed(), AFTER_A_FAILED_ACCEPT * 2);
}
