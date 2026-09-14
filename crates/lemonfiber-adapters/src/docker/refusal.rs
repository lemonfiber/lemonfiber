//! Telling one way of not reaching an engine from another.
//!
//! For a daemon on this machine there is one question — is Docker running — and
//! the detail field is the whole of the answer. For a daemon on another machine
//! there are three, and they have nothing to do with each other: a name that goes
//! nowhere is a typo or a nameserver, a port that declines is a daemon that is not
//! listening where the endpoint says, and a login that is refused is a key the
//! other machine does not hold. Reported as one, every one of them tells an
//! operator to start Docker Desktop on a laptop whose Docker Desktop is running.
//!
//! Read from the transport's own words rather than from a typed error, because the
//! condition is not in the type: a connection failure arrives through several
//! layers of client library as one variant, and only the sentence at the bottom of
//! the chain says which of the three it was. Kept as a function over text so every
//! branch is reachable from a test with no network, which is the one place these
//! could otherwise never be exercised.

use lemonfiber_ports::docker::{Failure, Reach, Target};

/// What a name that could not be turned into an address is said to be.
///
/// Spelled as several platforms spell it. The resolver's wording belongs to the
/// platform, and a list that only knew Linux's would report a Mac's unresolvable
/// host as a refused connection.
const UNRESOLVED: [&str; 6] = [
    "could not resolve hostname",
    "name or service not known",
    "nodename nor servname",
    "failed to lookup address",
    "temporary failure in name resolution",
    "no such host",
];

/// What a machine that would not take the login is said to have said.
///
/// Only consulted for an endpoint reached over SSH. "Permission denied" is also
/// what a local socket says to a user who is not in the docker group, and calling
/// that an authentication failure would send an operator looking for a key.
const REJECTED: [&str; 5] = [
    "permission denied",
    "publickey",
    "host key verification failed",
    "too many authentication failures",
    "authentication failed",
];

/// What a machine that was found and did not accept a connection is said to say.
const REFUSED: [&str; 3] = ["connection refused", "econnrefused", "no route to host"];

/// Which condition the transport described, given where the engine is.
///
/// A local engine keeps the one answer it has always had. The distinctions only
/// exist for a daemon that is somewhere else, and inventing them for a socket on
/// this filesystem would report a stopped Docker Desktop as a refused connection
/// to a host nobody named.
pub(super) fn classify(target: &Target, reason: &str) -> Failure {
    let Some(host) = target.host() else {
        return Failure::Unreachable {
            reason: reason.to_owned(),
        };
    };
    let said = reason.to_ascii_lowercase();
    let over_ssh = matches!(target.reach, Reach::Ssh(_));
    let reason = quoted(target, reason, &host);

    if names(&said, &UNRESOLVED) {
        return Failure::Unresolved { host, reason };
    }
    if over_ssh && names(&said, &REJECTED) {
        return Failure::Rejected { host, reason };
    }
    if names(&said, &REFUSED) {
        return Failure::Refused { host, reason };
    }
    // Everything else against a remote host: the machine did not answer, and which
    // of the several ways that can happen is not knowable from here. Its own answer
    // rather than the local one, because the local one tells the operator to start
    // Docker on the machine they are sitting at — which is running.
    Failure::Unanswered { host, reason }
}

/// The transport's own words, with the endpoint it quoted written as it may be shown.
///
/// A client that cannot connect commonly repeats the address it was given, and the
/// address it was given is the raw one — with the password a URL may carry in front
/// of its host still in it. The summary is built from the withheld form already; this
/// is the other half, and without it the detail beside that summary would print what
/// the summary was careful not to.
///
/// A plain substitution rather than a rule about credentials, because there is no
/// guessing to do: the one string that must not appear is known exactly, and so is
/// what it should read as instead.
fn quoted(target: &Target, reason: &str, host: &str) -> String {
    match target.endpoint() {
        Some(endpoint) if endpoint != host => reason.replace(endpoint, host),
        Some(_) | None => reason.to_owned(),
    }
}

/// Whether any of these phrases appears in what the transport said.
fn names(said: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| said.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::classify;
    use lemonfiber_ports::docker::{Origin, Target};
    use lemonfiber_ports::error::Diagnose as _;

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
}
