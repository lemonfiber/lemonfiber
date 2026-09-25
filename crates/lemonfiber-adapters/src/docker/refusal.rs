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
mod tests;
