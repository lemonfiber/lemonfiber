//! Working out which engine this run is pointed at, from what Docker records.
//!
//! The precedence is Docker's own and is decided in the port beside the vocabulary,
//! where a test can drive all four orders without a home directory. What is left
//! here is the reading: two files the Docker command line writes, in a format it
//! owns, in a directory only this side of the boundary knows how to find.
//!
//! It lives in the adapter rather than behind the filesystem port for two reasons
//! that are the same reason. The answer is needed before a context exists — the
//! engine client and the Compose invocation are both built from it, and a port is
//! something a built context already holds — and reading somebody else's format is
//! exactly the translation this crate is for. The part that could be wrong, which
//! is the order the three sources are consulted in, is not here.
//!
//! Contexts are found by reading each recorded one and comparing the name it
//! carries, rather than by hashing the name to find its directory. Docker's layout
//! puts a context under the digest of its name, and reproducing that hash here
//! would be this crate keeping a second opinion about somebody else's storage
//! scheme — one that fails silently, and only for the operators who use contexts.

use std::path::{Path, PathBuf};

use lemonfiber_ports::docker::{chosen, Choice, Origin, Reach, Target};

/// The variable naming an endpoint outright.
const HOST: &str = "DOCKER_HOST";

/// The variable naming a context.
const CONTEXT: &str = "DOCKER_CONTEXT";

/// The variable asking that a TCP connection be verified with TLS.
const TLS_VERIFY: &str = "DOCKER_TLS_VERIFY";

/// The variable naming where Docker keeps its own configuration.
const CONFIGURATION: &str = "DOCKER_CONFIG";

/// Where Docker keeps its configuration beneath a home directory.
const BENEATH_HOME: &str = ".docker";

/// Which engine this run is pointed at, read from this process's environment.
///
/// The one place the environment is read, so everything below it is a function of
/// values a test supplies.
#[must_use]
pub fn from_environment() -> Target {
    resolved(
        said(HOST).as_deref(),
        said(CONTEXT).as_deref(),
        said(TLS_VERIFY).is_some(),
        configuration().as_deref(),
    )
}

/// Which engine these values point at.
///
/// `docker` is where Docker keeps its own configuration, absent on a machine that
/// will not say where a home directory is — which reads as an operator with no
/// contexts rather than as a failure, because a machine with no home directory has
/// nowhere for Docker to have written one.
#[must_use]
pub(crate) fn resolved(
    host: Option<&str>,
    context: Option<&str>,
    tls: bool,
    docker: Option<&Path>,
) -> Target {
    let target = match chosen(host, context, current(docker).as_deref()) {
        Choice::Defaults => Target::local(),
        Choice::Endpoint(endpoint) => Target::at(&endpoint, Origin::Variable),
        Choice::Named(name) => under(&name, docker),
    };
    if tls {
        return unverifiable(target);
    }
    target
}

/// The same target, refused where the operator asked for a guarantee this cannot give.
///
/// Asking for the connection to be verified and being connected to in the clear is
/// the one outcome nobody wants: the operator believes the traffic is authenticated
/// and it is not. This build carries no certificate handling for the engine, so the
/// honest answer is that the endpoint cannot be driven rather than a connection that
/// quietly means something else.
fn unverifiable(target: Target) -> Target {
    let Reach::Tcp(endpoint) = &target.reach else {
        return target;
    };
    Target {
        reach: Reach::Beyond(endpoint.clone()),
        origin: target.origin,
    }
}

/// The endpoint a named context points at, or the plain fact that there is no such
/// context.
fn under(name: &str, docker: Option<&Path>) -> Target {
    match recorded(name, docker) {
        Some(endpoint) => Target::at(&endpoint, Origin::Context(name.to_owned())),
        None => Target::missing(name),
    }
}

/// Where Docker keeps its configuration for this operator, where that can be said.
fn configuration() -> Option<PathBuf> {
    beneath(said(CONFIGURATION), said("HOME"))
}

/// Where Docker keeps its configuration, from what the environment said.
///
/// Apart from the reading so that both answers can be driven without setting a
/// variable for the whole process. A test that exported one would be deciding for
/// every test beside it, and the branch that goes untested is the one an operator
/// who has moved their Docker configuration depends on entirely.
///
/// `DOCKER_CONFIG` is taken as it stands; otherwise it is the conventional directory
/// beneath the home. Nowhere at all where the platform will say neither, which reads
/// as an operator with no contexts rather than as a failure.
fn beneath(configured: Option<String>, home: Option<String>) -> Option<PathBuf> {
    if let Some(named) = configured {
        return Some(PathBuf::from(named));
    }
    home.map(|home| PathBuf::from(home).join(BENEATH_HOME))
}

/// A variable that says something, which an unset or empty one does not.
fn said(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// The context Docker's own configuration records as current, where it records one.
fn current(docker: Option<&Path>) -> Option<String> {
    let text = std::fs::read_to_string(docker?.join("config.json")).ok()?;
    named_context(&text)
}

/// The endpoint recorded for a context of this name, where one is recorded.
///
/// Every recorded context is read rather than one being looked up, because the
/// directory each lives in is named for a digest of its own name. See the note at
/// the top of this file for why that digest is not reproduced here.
fn recorded(name: &str, docker: Option<&Path>) -> Option<String> {
    let meta = docker?.join("contexts").join("meta");
    for entry in std::fs::read_dir(meta).ok()?.flatten() {
        let Ok(text) = std::fs::read_to_string(entry.path().join("meta.json")) else {
            continue;
        };
        if let Some(endpoint) = endpoint_of(&text, name) {
            return Some(endpoint);
        }
    }
    None
}

/// The current context named in Docker's configuration document.
///
/// Apart from the reading of the file so that what a malformed, empty or
/// unexpectedly-shaped document comes to is exercised without writing one.
fn named_context(text: &str) -> Option<String> {
    let document: serde_json::Value = serde_json::from_str(text).ok()?;
    document
        .get("currentContext")?
        .as_str()
        .map(str::to_owned)
        .filter(|name| !name.trim().is_empty())
}

/// The Docker endpoint this recorded context describes, where it is the one named.
fn endpoint_of(text: &str, name: &str) -> Option<String> {
    let document: serde_json::Value = serde_json::from_str(text).ok()?;
    if document.get("Name")?.as_str()? != name {
        return None;
    }
    document
        .get("Endpoints")?
        .get("docker")?
        .get("Host")?
        .as_str()
        .map(str::to_owned)
        .filter(|endpoint| !endpoint.trim().is_empty())
}

#[cfg(test)]
mod tests;
