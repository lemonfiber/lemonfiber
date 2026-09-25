use super::classify;
use lemonfiber_error::Diagnose as _;
use lemonfiber_ports::docker::{Origin, Target};

/// The endpoint each condition is exercised against, since two of the three are
/// only told apart for a host reached over SSH.
fn over_ssh() -> Target {
    Target::at("ssh://media@nas.local", Origin::Variable)
}

/// The code an operator would be given for what these words were read as.
///
/// Compared as a value rather than matched as a shape. A `matches!` inside an
/// assertion leaves the arm that did not match as a region no passing run ever
/// enters, and the coverage gate counts those — correctly, since a branch nothing
/// reaches is a branch nothing checked. The code is also the more useful thing to
/// pin: it is what an operator searches for, and it is what must not change.
fn code(target: &Target, said: &str) -> String {
    classify(target, said).problem().code.to_string()
}

#[test]
fn a_name_that_goes_nowhere_is_not_a_refused_connection() {
    for said in [
        "ssh: Could not resolve hostname nas.local",
        "failed to lookup address information: Name or service not known",
        "dial tcp: lookup nas.local: no such host",
    ] {
        assert_eq!(code(&over_ssh(), said), "DOCKER-3", "{said}");
    }
}

#[test]
fn a_key_the_other_machine_will_not_take_is_its_own_answer() {
    for said in [
        "media@nas.local: Permission denied (publickey).",
        "Host key verification failed.",
    ] {
        assert_eq!(code(&over_ssh(), said), "DOCKER-5", "{said}");
    }
}

#[test]
fn a_port_that_declines_is_told_apart_from_both() {
    let over_tcp = Target::at("tcp://nas.local:2375", Origin::Variable);
    let said = "error trying to connect: Connection refused (os error 61)";
    assert_eq!(code(&over_tcp, said), "DOCKER-4");
}

/// A local socket says "permission denied" to a user outside the docker group,
/// and that is not an authentication failure anybody can fix with a key.
#[test]
fn a_local_engine_keeps_the_one_answer_it_has_always_had() {
    let on_this_filesystem = Target::socket("/var/run/docker.sock");
    for said in [
        "permission denied while trying to connect to the Docker daemon socket",
        "Connection refused (os error 61)",
        "Could not resolve hostname",
    ] {
        assert_eq!(code(&Target::local(), said), "DOCKER-1", "{said}");
        assert_eq!(
            code(&on_this_filesystem, said),
            "DOCKER-1",
            "a socket on this filesystem is not a remote host: {said}"
        );
    }
}

/// The same words mean different things over the two remote transports.
#[test]
fn a_refused_permission_over_tcp_is_not_a_refused_key() {
    let over_tcp = Target::at("tcp://nas.local:2375", Origin::Variable);
    assert_eq!(
        code(&over_tcp, "permission denied"),
        "DOCKER-8",
        "there is no key in a TCP endpoint to have been refused"
    );
}

/// The detail beside the summary must not print what the summary withheld.
///
/// A client that cannot connect repeats the address it was handed, and the
/// address it was handed is the raw one. Both halves of what an operator reads
/// go through the same withholding, or only one of them is careful.
///
/// Read through the failure's own sentence, which carries the host and the
/// transport's words together. Taking the variant apart would mean an arm for the
/// shape it is not, and that arm is a region a passing run never enters.
#[test]
fn a_password_the_transport_echoed_back_is_not_printed_either() {
    let carried = Target::at("ssh://media:hunter2@nas.local", Origin::Variable);
    let said = classify(
        &carried,
        "ssh: connect to ssh://media:hunter2@nas.local: Connection refused",
    )
    .to_string();

    assert!(
        !said.contains("hunter2"),
        "the transport repeated the address it was handed, password and all"
    );
    assert!(
        said.contains("nas.local"),
        "the host went with the password, leaving nothing to act on"
    );
    assert!(
        said.contains("refused the connection"),
        "and it is still read as the refusal it was"
    );
}

/// A remote condition nothing here recognises still names the host.
#[test]
fn an_unrecognised_remote_failure_does_not_become_a_local_one() {
    let said = classify(&over_ssh(), "the stream ended unexpectedly").to_string();

    assert!(said.contains("nas.local"), "{said}");
    assert!(
        said.contains("did not answer"),
        "an unrecognised remote failure is its own answer, not the local one"
    );
}
