use super::{chained, connect, refused_exec, spoken, Failure};
use lemonfiber_ports::docker::{Origin, Target};

/// An error with an optional cause, for driving a chain without a socket.
///
/// The chains that matter here come out of a client library several layers deep,
/// and producing a real one takes a real connection that fails a real way. What
/// is actually being decided — whether a cause says something the sentence above
/// it did not — needs neither, so it is driven over the standard trait instead.
#[derive(Debug)]
struct Said {
    text: String,
    under: Option<Box<Said>>,
}

impl Said {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            under: None,
        }
    }

    fn caused_by(mut self, under: Self) -> Self {
        self.under = Some(Box::new(under));
        self
    }
}

impl std::fmt::Display for Said {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

impl std::error::Error for Said {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.under
            .as_deref()
            .map(|under| under as &(dyn std::error::Error + 'static))
    }
}

/// The condition is at the bottom of the chain, so the chain is what is read.
///
/// Both shapes are here because both are ordinary. A transparent wrapper renders
/// exactly what it wraps, and appending that again would say one sentence twice;
/// a wrapper with its own words hides the condition until the cause under it is
/// reached, which is the whole reason this walks rather than reading the top.
#[test]
fn what_a_failure_said_carries_its_causes_and_says_nothing_twice() {
    let quoting =
        Said::new("client error: connection refused").caused_by(Said::new("connection refused"));
    assert_eq!(chained(&quoting), "client error: connection refused");

    let its_own_words = Said::new("error trying to connect")
        .caused_by(Said::new("tcp connect error").caused_by(Said::new("Connection refused")));
    assert_eq!(
        chained(&its_own_words),
        "error trying to connect: tcp connect error: Connection refused"
    );

    assert_eq!(chained(&Said::new("nothing under it")), "nothing under it");
}

/// A client is built for every transport this build can drive, and for none of
/// the endpoints it cannot.
///
/// Building one connects to nothing, which is what lets both remote transports be
/// exercised here with no server on either. What the refused half proves is the
/// property the target exists for: an endpoint the reads cannot use is turned
/// away before a client exists, so it can never become one the writes use.
#[test]
fn a_client_is_built_for_each_transport_and_for_no_endpoint_beyond_them() {
    for endpoint in ["tcp://127.0.0.1:2375", "ssh://media@nas.local"] {
        let built = connect(&Target::at(endpoint, Origin::Variable)).is_ok();
        assert!(built, "{endpoint}");
    }

    for beyond in [
        Target::missing("nas"),
        Target::at("https://nas.local:2376", Origin::Variable),
    ] {
        let turned_away = connect(&beyond).is_err();
        assert!(turned_away, "{beyond:?}");
    }
}

/// Both refusals, one of which needs a daemon that answers badly.
///
/// The 404 is the only one an operator can act on differently, and it is the one a
/// real run produces. Everything else is the daemon being unreachable in some way,
/// which is reachable here and nowhere else.
#[test]
fn a_container_that_is_not_there_is_told_apart_from_a_daemon_that_is_not_well() {
    let missing = refused_exec(
        bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            message: "no such container".to_owned(),
        },
        "sonarr",
    );
    assert!(
        matches!(missing, Failure::NoSuchContainer { ref name } if name == "sonarr"),
        "got: {missing:?}"
    );

    let otherwise = refused_exec(
        bollard::errors::Error::DockerResponseServerError {
            status_code: 500,
            message: "it is not well".to_owned(),
        },
        "sonarr",
    );
    assert!(
        !matches!(otherwise, Failure::NoSuchContainer { .. }),
        "a daemon fault is not a missing container: {otherwise:?}"
    );
}

/// An exec nobody attached to wrote nothing, which is not the same as an error.
#[tokio::test]
async fn an_exec_that_was_not_attached_to_says_nothing() {
    let said = spoken(bollard::exec::StartExecResults::Detached).await;
    assert_eq!(said, String::new());
}

/// What an exec attached to a stream of these chunks came to.
///
/// Through the exec rather than through the loop underneath it, and that is the
/// point rather than a convenience: the loop is generic over the stream it reads,
/// so handing it a stream written here would compile a second copy of it and
/// measure that one instead of the one a run uses.
async fn attached(
    chunks: Vec<Result<bollard::container::LogOutput, bollard::errors::Error>>,
) -> String {
    spoken(bollard::exec::StartExecResults::Attached {
        output: Box::pin(tokio_stream::iter(chunks)),
        input: Box::pin(tokio::io::sink()),
    })
    .await
}

/// An attached exec is read through to the end, in the order it arrived.
#[tokio::test]
async fn every_chunk_an_exec_sent_is_gathered_in_order() {
    let said = attached(vec![
        Ok(bollard::container::LogOutput::StdOut {
            message: "one\n".into(),
        }),
        Ok(bollard::container::LogOutput::StdErr {
            message: "two\n".into(),
        }),
    ])
    .await;
    assert_eq!(said, "one\ntwo\n");
}

/// And a chunk that will not arrive ends it, rather than being passed over.
///
/// The break stops the reading rather than being skipped: what arrived before it
/// comes back, and what the stream would have said afterwards does not. Passing
/// over a break and carrying on would splice two halves of an answer together
/// with the gap invisible, which is the one shape worse than a short answer —
/// the leak check compares what two of these said, and it can tell a short
/// answer from an address.
///
/// That this is not itself a refusal is [`gathered`]'s own reasoning: the exec's
/// exit status is asked for immediately afterwards, and it is what says whether
/// what arrived is an answer.
#[tokio::test]
async fn a_chunk_that_will_not_arrive_ends_the_gathering() {
    let said = attached(vec![
        Ok(bollard::container::LogOutput::StdOut {
            message: "some of it\n".into(),
        }),
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 500,
            message: "the stream stopped".to_owned(),
        }),
        Ok(bollard::container::LogOutput::StdOut {
            message: "and the rest\n".into(),
        }),
    ])
    .await;
    assert_eq!(said, "some of it\n", "the reading stops at the break");
}
