//! Reaching the engine this machine is configured for, and telling its failures apart.

use super::fake;
use lemonfiber_adapters::Daemon;
use lemonfiber_ports::docker::{Engine as _, Failure, Origin, Target};

#[tokio::test]
async fn the_engine_this_machine_is_configured_for_is_reachable_or_reported_absent() {
    // Whether a daemon is running here is not this test's business. Either
    // answer is correct; what must never happen is the local address going
    // unexercised, or an absent daemon being reported as something an operator
    // would go looking for a container about.
    let daemon = Daemon::local();
    assert!(
        format!("{daemon:?}").contains("Local"),
        "an adapter has to say which engine it is pointed at, for a bundle to be worth reading"
    );

    let outcome = daemon.list("lemonfiber-no-such-project").await;
    assert!(
        matches!(outcome, Ok(_) | Err(Failure::Unreachable { .. })),
        "{outcome:?}"
    );
}

/// A client is built for each remote transport by the run that uses one.
///
/// The endpoint's scheme picks the client, one arm per transport, and the two remote
/// arms arrived with remote operation. They had a test beside the code and nothing
/// driving them through the adapter a run reaches — and this library is compiled
/// twice, once into its own test binary and once as the dependency these integration
/// tests link, so an arm entered only in the first is an arm no run has been shown to
/// take.
///
/// Neither endpoint answers and neither is meant to. A client that could not be built
/// at all is refused before a request is made — that is the other rule this adapter
/// holds — so a refusal that arrives *from the request* is the proof that the client
/// was built, and it names the endpoint it was built for. Both point at a port on this
/// machine that nothing listens on, so the answer arrives at once rather than after
/// the connection timeout, and nothing leaves the machine.
#[tokio::test]
async fn a_client_is_built_for_each_remote_transport_by_a_run_that_uses_one() {
    for endpoint in ["tcp://127.0.0.1:1", "ssh://nobody@127.0.0.1:1"] {
        let refused = Daemon::reaching(Target::at(endpoint, Origin::Variable))
            .list("lemonfiber")
            .await;

        let said = format!("{refused:?}");
        assert!(
            refused.is_err(),
            "{endpoint} answered, and nothing is listening there: {said}"
        );
        assert!(
            said.contains(endpoint),
            "a refusal from somewhere else says which somewhere: {said}"
        );
    }
}

/// The two answers the location pre-flight is built on, read off a real exchange.
///
/// There is no route that stats a path on the host, so the question goes as a
/// request to create a container against an image that cannot exist: the daemon
/// checks the mounts first, and which of the two refusals comes back is the answer.
/// Driven against a socket rather than against the classifier alone because the
/// shape being relied on is the daemon's, and a test that only called the reading
/// function would prove the reading and not the asking.
#[cfg(unix)]
#[tokio::test]
async fn a_path_the_machine_has_not_got_is_told_from_one_it_has() {
    use lemonfiber_ports::docker::{Locations as _, Presence};
    use std::path::Path;

    let cases = [
        (
            "absent",
            400,
            r#"{"message":"invalid mount config for type \"bind\": bind source path does not exist: /srv/media"}"#,
            Presence::Absent,
        ),
        (
            "past the mounts and looking for the image",
            404,
            r#"{"message":"No such image: sha256:0000"}"#,
            Presence::There,
        ),
        (
            "something else entirely",
            500,
            r#"{"message":"engine is having a day"}"#,
            Presence::Unknown,
        ),
    ];

    for (what, status, body, expected) in cases {
        let engine = fake::engine(
            "located",
            vec![(
                "containers/create",
                fake::Reply::Body(status, body.to_owned()),
            )],
        );

        let answer = Daemon::at(&engine.socket)
            .located(Path::new("/srv/media"))
            .await;

        assert_eq!(answer.ok(), Some(expected), "{what}");
        engine.stop().await;
    }
}

/// A daemon that made the container is one that no longer knows what it measured.
///
/// It cannot happen: the request names an image identified by a digest of all
/// zeroes, which is not the digest of anything. But "cannot happen" is a claim about
/// today's daemon, and the answer if it ever did is the one thing this must never
/// say — that the path is there. A creation that succeeded proves the mount was not
/// what the refusal was about, and therefore proves nothing.
#[cfg(unix)]
#[tokio::test]
async fn a_daemon_that_somehow_made_the_container_has_told_us_nothing() {
    use lemonfiber_ports::docker::{Locations as _, Presence};
    use std::path::Path;

    let engine = fake::engine(
        "created",
        vec![(
            "containers/create",
            fake::Reply::Body(201, r#"{"Id":"made-one","Warnings":[]}"#.to_owned()),
        )],
    );

    let answer = Daemon::at(&engine.socket)
        .located(Path::new("/srv/media"))
        .await;

    assert_eq!(answer.ok(), Some(Presence::Unknown));
    engine.stop().await;
}

/// A daemon that answered with something the client cannot read is a transport
/// failure, not a verdict on the path.
///
/// Apart from the three readable answers because it arrives as a different kind of
/// error entirely — nothing came back that could be classified — and apart from the
/// machine that never answered because that one fails before the question is even
/// sent. This is the gap between them: connected, asked, and then nothing usable.
#[cfg(unix)]
#[tokio::test]
async fn a_daemon_whose_answer_cannot_be_read_is_reported_as_the_transport_failing() {
    use lemonfiber_ports::docker::Locations as _;
    use std::path::Path;

    let engine = fake::engine(
        "unreadable",
        vec![(
            "containers/create",
            fake::Reply::Body(200, "this is not the document it promised".to_owned()),
        )],
    );

    let answer = Daemon::at(&engine.socket)
        .located(Path::new("/srv/media"))
        .await;

    assert!(
        matches!(answer, Err(Failure::Unreachable { .. })),
        "an answer nobody can read is not evidence about a path: {answer:?}"
    );
    engine.stop().await;
}

/// A machine that never answered said nothing about the path.
///
/// Kept apart from the three answers above because it is a different kind of fact:
/// the distinctions the connection draws are worth keeping, and folding an
/// unreachable daemon into "cannot tell" would lose them at the one moment they are
/// most useful.
#[cfg(unix)]
#[tokio::test]
async fn a_machine_that_never_answered_is_a_failure_rather_than_an_answer() {
    use lemonfiber_ports::docker::Locations as _;
    use std::path::Path;

    let nowhere = std::path::PathBuf::from("/tmp/lf-no-such-engine.sock");
    let refused = Daemon::at(&nowhere).located(Path::new("/srv/media")).await;

    assert!(
        matches!(refused, Err(Failure::Unreachable { .. })),
        "{refused:?}"
    );
}
