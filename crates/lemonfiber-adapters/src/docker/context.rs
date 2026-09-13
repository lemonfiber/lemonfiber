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
    if let Some(named) = said(CONFIGURATION) {
        return Some(PathBuf::from(named));
    }
    said("HOME").map(|home| PathBuf::from(home).join(BENEATH_HOME))
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
mod tests {
    use super::{endpoint_of, named_context, resolved, unverifiable};
    use lemonfiber_ports::docker::{Origin, Reach, Target};
    use std::path::{Path, PathBuf};

    /// A scratch directory laid out the way Docker lays its own out.
    ///
    /// A real directory rather than a fake, because what is being exercised is the
    /// reading of somebody else's layout — a fake here would be asserting that this
    /// file agrees with itself about a format neither of them owns.
    struct Recorded(PathBuf);

    impl Recorded {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("lemonfiber-context-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let _ = std::fs::create_dir_all(root.join("contexts").join("meta"));
            Self(root)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        /// Record a context the way the Docker command line records one.
        fn context(&self, digest: &str, name: &str, endpoint: &str) -> &Self {
            let under = self.0.join("contexts").join("meta").join(digest);
            let _ = std::fs::create_dir_all(&under);
            let _ = std::fs::write(
                under.join("meta.json"),
                format!(r#"{{"Name":"{name}","Endpoints":{{"docker":{{"Host":"{endpoint}"}}}}}}"#),
            );
            self
        }

        fn current(&self, name: &str) -> &Self {
            let _ = std::fs::write(
                self.0.join("config.json"),
                format!(r#"{{"currentContext":"{name}"}}"#),
            );
            self
        }
    }

    impl Drop for Recorded {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The whole order, driven over a directory laid out as Docker lays one out.
    #[test]
    fn the_variable_wins_then_the_named_context_then_the_recorded_one() {
        let recorded = Recorded::new("order");
        recorded
            .context("aaa", "nas", "ssh://media@nas.local")
            .context("bbb", "shed", "tcp://shed.local:2375")
            .current("shed");
        let at = Some(recorded.path());

        assert_eq!(
            resolved(Some("ssh://laptop"), Some("nas"), false, at).endpoint(),
            Some("ssh://laptop"),
            "the variable beats both"
        );
        let named = resolved(None, Some("nas"), false, at);
        assert_eq!(named.endpoint(), Some("ssh://media@nas.local"));
        assert_eq!(named.context(), Some("nas"));

        let fallen_back = resolved(None, None, false, at);
        assert_eq!(fallen_back.endpoint(), Some("tcp://shed.local:2375"));
        assert_eq!(fallen_back.context(), Some("shed"));
    }

    /// A context nobody recorded is refused rather than quietly answered locally.
    #[test]
    fn a_context_this_machine_does_not_have_is_not_the_local_daemon() {
        let recorded = Recorded::new("missing");
        recorded.context("aaa", "nas", "ssh://media@nas.local");

        let target = resolved(None, Some("typo"), false, Some(recorded.path()));
        assert_eq!(target.reach, Reach::Missing("typo".to_owned()));
        assert!(target.refusal().is_some(), "{target:?}");
        assert_eq!(target.endpoint(), None, "nothing is handed to Compose");
    }

    /// Nothing recorded anywhere is this machine's own daemon, which is the ordinary
    /// case and must stay silent.
    #[test]
    fn a_machine_with_no_contexts_at_all_is_this_machines_own_daemon() {
        let empty = Recorded::new("empty");
        let target = resolved(None, None, false, Some(empty.path()));
        assert_eq!(target, Target::local());
        assert_eq!(resolved(None, None, false, None), Target::local());
    }

    /// A document that is not what this expects contributes nothing rather than
    /// half an answer.
    #[test]
    fn a_recorded_context_that_cannot_be_read_names_no_endpoint() {
        assert_eq!(
            named_context(r#"{"currentContext":"nas"}"#).as_deref(),
            Some("nas")
        );
        assert_eq!(named_context("{}"), None);
        assert_eq!(named_context("not json at all"), None);
        assert_eq!(named_context(r#"{"currentContext":""}"#), None);
        assert_eq!(named_context(r#"{"currentContext":7}"#), None);

        let good = r#"{"Name":"nas","Endpoints":{"docker":{"Host":"ssh://nas"}}}"#;
        assert_eq!(endpoint_of(good, "nas").as_deref(), Some("ssh://nas"));
        assert_eq!(endpoint_of(good, "other"), None, "a different context");
        assert_eq!(endpoint_of(r#"{"Name":"nas"}"#, "nas"), None);
        assert_eq!(
            endpoint_of(r#"{"Name":"nas","Endpoints":{"docker":{}}}"#, "nas"),
            None
        );
        assert_eq!(endpoint_of("{", "nas"), None);
    }

    /// Asking for a verified connection and getting an unverified one is the one
    /// outcome that must not happen quietly.
    #[test]
    fn a_connection_that_was_to_be_verified_is_refused_rather_than_made_in_the_clear() {
        let recorded = Recorded::new("tls");
        recorded.context("aaa", "nas", "tcp://nas.local:2376");

        let target = resolved(None, Some("nas"), true, Some(recorded.path()));
        assert!(matches!(target.reach, Reach::Beyond(_)), "{target:?}");
        assert!(target.refusal().is_some());

        // Only TCP is affected: a socket and an SSH endpoint carry their own
        // guarantees and the variable says nothing about either.
        let over_ssh = Target::at("ssh://nas.local", Origin::Variable);
        assert_eq!(unverifiable(over_ssh.clone()), over_ssh);
        assert_eq!(unverifiable(Target::local()), Target::local());
    }
}
