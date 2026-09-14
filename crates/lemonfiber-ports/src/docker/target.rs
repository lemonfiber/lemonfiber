//! Which container engine a run is pointed at, and how it came to be that one.
//!
//! Vocabulary rather than resolution. Reading Docker's own files to find out which
//! context is current is filesystem work and lives in the adapter; what a resolved
//! answer *is*, and what may be said about it, is decided here — where a test can
//! drive every endpoint shape without a daemon, a home directory or a network.
//!
//! One value answers for the whole run. The Engine API is reached by building a
//! client from it and Compose is driven by passing it as `--host`, so the two
//! cannot be pointed at different machines by anything short of constructing a
//! second one — which is the failure this type exists to make visible. Sending the
//! reads to one daemon and the writes to another is worse than refusing the remote
//! case outright, because the operator is shown a stack that is not the one they
//! are changing.

use super::Failure;
use crate::withheld::without_credentials;

/// The context name Docker gives the machine's own defaults.
///
/// Named explicitly because it is not merely one context among others: selecting it
/// stops the search rather than continuing it, so an operator who sets it is asking
/// for the local daemon and must not be given whatever a configuration file still
/// remembers.
pub const DEFAULT_CONTEXT: &str = "default";

/// Where the engine a run operates is listening.
///
/// The endpoint is kept whole rather than split into its parts, because every
/// consumer wants it whole: the client library parses it, the Docker command line
/// takes it as one argument, and a refusal quotes it. Splitting it here would mean
/// reassembling it three times, and three reassemblies are three ways to differ.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Reach {
    /// Whatever this machine's own conventions say, which is the ordinary case.
    #[default]
    Local,
    /// A socket on this machine's filesystem.
    Socket(String),
    /// A daemon answering over TCP.
    Tcp(String),
    /// A daemon reached over SSH, which is how a laptop reaches a server.
    Ssh(String),
    /// An endpoint this build cannot drive, kept whole so the refusal names it.
    ///
    /// A scheme nothing here speaks, and a TCP endpoint the operator has asked to
    /// be verified with TLS — this build carries no certificate handling for the
    /// engine, and connecting in the clear to a socket expecting TLS would be a
    /// confusing transport error rather than the plain refusal it deserves.
    Beyond(String),
    /// A context was named and Docker records no endpoint under that name.
    ///
    /// Its own answer rather than a quiet fall back to this machine's daemon. A
    /// mistyped context that silently became the local engine is how an operator
    /// comes to stop the wrong stack while reading the right one.
    Missing(String),
}

/// How a run came to be pointed at the engine it is pointed at.
///
/// Carried because the two remote paths are told apart by nothing else, and an
/// operator who did not realise a context was in force is owed the name of the
/// thing that set it: a shell variable is something they can see, and a context
/// recorded in a configuration file months ago is not.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Origin {
    /// Nothing said otherwise, so this machine's own daemon.
    #[default]
    Defaults,
    /// `DOCKER_HOST` named the endpoint outright.
    Variable,
    /// A named Docker context, whether the environment or the configuration chose it.
    Context(String),
}

/// The engine this run talks to, and how it was chosen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Target {
    /// Where the engine is listening.
    pub reach: Reach,
    /// How this run came to be pointed there.
    pub origin: Origin,
}

impl Target {
    /// This machine's own daemon, reached however this machine says.
    #[must_use]
    pub fn local() -> Self {
        Self::default()
    }

    /// The engine at this endpoint, chosen this way.
    ///
    /// An endpoint that is empty or only whitespace is nothing said, which is the
    /// local daemon — a shell that exports an empty `DOCKER_HOST` has not asked for
    /// a remote one, and treating it as an endpoint it could not read would refuse a
    /// machine that is working perfectly.
    #[must_use]
    pub fn at(endpoint: &str, origin: Origin) -> Self {
        let said = endpoint.trim();
        if said.is_empty() {
            return Self::local();
        }
        Self {
            reach: reach(said),
            origin,
        }
    }

    /// A socket this run was pointed at directly.
    ///
    /// The seam a test points the adapter at an engine it wrote itself through, and
    /// the shape `DOCKER_HOST=unix://…` resolves to.
    #[must_use]
    pub fn socket(path: &str) -> Self {
        Self {
            reach: Reach::Socket(qualified(path)),
            origin: Origin::Variable,
        }
    }

    /// Whether this run is operating a daemon that is not this machine's.
    ///
    /// A socket is not remote however it was named: it is a path on this filesystem,
    /// so the stack's own paths resolve where the operator is sitting and nothing
    /// about the remote trap applies.
    #[must_use]
    pub fn is_remote(&self) -> bool {
        !matches!(self.reach, Reach::Local | Reach::Socket(_))
    }

    /// The endpoint, as both the client library and the Docker command line take it.
    ///
    /// `None` where nothing was chosen, which is the one case that must stay silent:
    /// passing a guessed endpoint to Compose would override a machine's own
    /// conventions with this build's idea of them.
    #[must_use]
    pub fn endpoint(&self) -> Option<&str> {
        match &self.reach {
            Reach::Local | Reach::Missing(_) => None,
            Reach::Socket(url) | Reach::Tcp(url) | Reach::Ssh(url) | Reach::Beyond(url) => {
                Some(url)
            }
        }
    }

    /// Why nothing may be done against this engine, where nothing may.
    ///
    /// The one question both halves of the run ask before they act, so that an
    /// endpoint the reads cannot use can never become one the writes do. Answering
    /// it in two places would be two opinions about the same value, and the
    /// disagreement would not surface until something had already been changed on a
    /// machine nobody was looking at.
    ///
    /// `None` for every endpoint that can be driven, which is the ordinary case.
    #[must_use]
    pub fn refusal(&self) -> Option<Failure> {
        match &self.reach {
            Reach::Beyond(endpoint) => Some(Failure::Unsupported {
                endpoint: named(endpoint),
            }),
            Reach::Missing(name) => Some(Failure::NoSuchContext { name: name.clone() }),
            Reach::Local | Reach::Socket(_) | Reach::Tcp(_) | Reach::Ssh(_) => None,
        }
    }

    /// The context Docker was asked for and does not have, where that is what happened.
    #[must_use]
    pub fn missing(name: &str) -> Self {
        Self {
            reach: Reach::Missing(name.to_owned()),
            origin: Origin::Context(name.to_owned()),
        }
    }

    /// The host being operated on, as it may be shown.
    ///
    /// `None` for a local run, because there is nothing an operator could mistake:
    /// naming this machine on every line would be noise, and the requirement is
    /// about the case where a command reaches somewhere else.
    ///
    /// Withheld through the same rule every other quoted value passes through. An
    /// endpoint is a URL and a URL may carry a password in front of its host and a
    /// token in its query, so a name printed under every command is exactly the
    /// wrong place to reproduce one.
    #[must_use]
    pub fn host(&self) -> Option<String> {
        if !self.is_remote() {
            return None;
        }
        self.endpoint().map(named)
    }

    /// The context whose name put this run where it is, where a context did.
    #[must_use]
    pub fn context(&self) -> Option<&str> {
        match &self.origin {
            Origin::Context(name) => Some(name),
            Origin::Defaults | Origin::Variable => None,
        }
    }
}

/// Which endpoint a URL names, decided from its scheme alone.
///
/// The scheme is the whole of the question. What follows it belongs to the client
/// that will connect, and a second parser here would be a second opinion about
/// somebody else's address — wrong in exactly the cases nobody tests.
fn reach(endpoint: &str) -> Reach {
    let scheme = endpoint.split_once("://").map(|(scheme, _)| scheme);
    match scheme {
        Some("unix") => Reach::Socket(endpoint.to_owned()),
        Some("tcp" | "http") => Reach::Tcp(endpoint.to_owned()),
        Some("ssh") => Reach::Ssh(endpoint.to_owned()),
        // A bare path is how `DOCKER_HOST` is sometimes written, and it means a
        // socket. Qualified here so everything downstream sees one shape.
        None if endpoint.starts_with('/') => Reach::Socket(qualified(endpoint)),
        Some(_) | None => Reach::Beyond(endpoint.to_owned()),
    }
}

/// A socket path written the way every consumer of one expects it.
fn qualified(path: &str) -> String {
    if path.contains("://") {
        return path.to_owned();
    }
    format!("unix://{path}")
}

/// The endpoint as it may be shown, with anything a credential rides in taken out.
fn named(endpoint: &str) -> String {
    without_credentials(endpoint)
}

/// Which of the three ways of naming an engine won.
///
/// Docker's own order, stated once so nothing has to reconstruct it: the variable
/// beats the environment's context, which beats the one a configuration file
/// remembers. Kept apart from the reading of those values because the order is the
/// part that can be wrong, and it is the part a test can hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Nothing chose anything; this machine's own daemon.
    Defaults,
    /// An endpoint, named outright.
    Endpoint(String),
    /// A context, to be looked up by name.
    Named(String),
}

/// Which of the three ways of naming an engine applies, given what was found.
///
/// `current` is the context a configuration file records, which is the weakest of
/// the three and the one an operator is least likely to remember setting.
///
/// A context named `default` ends the search rather than continuing it. It means
/// the machine's own daemon, and falling through to the configuration file would
/// take an operator who explicitly asked for local and hand them a remote host.
#[must_use]
pub fn chosen(host: Option<&str>, context: Option<&str>, current: Option<&str>) -> Choice {
    if let Some(endpoint) = said(host) {
        return Choice::Endpoint(endpoint.to_owned());
    }
    if let Some(name) = said(context) {
        return named_or_default(name);
    }
    said(current).map_or(Choice::Defaults, named_or_default)
}

/// A context name as a choice, with the local one read as no choice at all.
fn named_or_default(name: &str) -> Choice {
    if name == DEFAULT_CONTEXT {
        Choice::Defaults
    } else {
        Choice::Named(name.to_owned())
    }
}

/// A value that says something, which an unset or empty variable does not.
fn said(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|said| !said.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{chosen, Choice, Origin, Reach, Target};

    #[test]
    fn nothing_chosen_is_this_machines_own_daemon() {
        let target = Target::local();
        assert!(!target.is_remote());
        assert_eq!(target.endpoint(), None);
        assert_eq!(target.host(), None);
        assert_eq!(target.context(), None);
        assert_eq!(Target::at("   ", Origin::Variable), target);
    }

    #[test]
    fn each_scheme_is_recognised_as_the_transport_it_names() {
        let cases = [
            ("unix:///var/run/docker.sock", Reach::Socket(String::new())),
            ("tcp://nas.local:2375", Reach::Tcp(String::new())),
            ("http://nas.local:2375", Reach::Tcp(String::new())),
            ("ssh://media@nas.local", Reach::Ssh(String::new())),
            ("wat://nas.local", Reach::Beyond(String::new())),
        ];
        for (endpoint, expected) in cases {
            let target = Target::at(endpoint, Origin::Variable);
            assert_eq!(
                std::mem::discriminant(&target.reach),
                std::mem::discriminant(&expected),
                "{endpoint} was read as {:?}",
                target.reach
            );
            assert_eq!(target.endpoint(), Some(endpoint), "kept whole: {endpoint}");
        }
    }

    /// A bare path is a socket, and it is qualified so one shape leaves here.
    #[test]
    fn a_bare_path_is_a_socket_and_is_written_as_one() {
        let target = Target::at("/var/run/docker.sock", Origin::Variable);
        assert_eq!(target.endpoint(), Some("unix:///var/run/docker.sock"));
        assert!(!target.is_remote(), "a socket is on this filesystem");
        assert_eq!(
            Target::socket("/tmp/test.sock").endpoint(),
            Some("unix:///tmp/test.sock")
        );
        assert_eq!(
            Target::socket("unix:///tmp/test.sock").endpoint(),
            Some("unix:///tmp/test.sock"),
            "a path already qualified is not qualified twice"
        );
    }

    /// A local run names no host, because there is nothing to mistake it for.
    #[test]
    fn only_a_remote_run_has_a_host_to_name() {
        assert_eq!(Target::local().host(), None);
        assert_eq!(
            Target::at("unix:///var/run/docker.sock", Origin::Variable).host(),
            None
        );
        assert_eq!(
            Target::at("ssh://media@nas.local", Origin::Variable).host(),
            Some("ssh://media@nas.local".to_owned())
        );
    }

    /// The name printed under every command is exactly the wrong place for a secret.
    #[test]
    fn a_credential_in_an_endpoint_is_not_shown() {
        let carried = Target::at("ssh://media:hunter2@nas.local", Origin::Variable);
        let shown = carried.host().unwrap_or_default();
        assert!(
            !shown.contains("hunter2"),
            "the password survived into what is shown"
        );
        assert!(
            shown.contains("nas.local"),
            "the host went with the password, leaving nothing to read"
        );

        let queried = Target::at("tcp://nas.local:2375?token=abc123", Origin::Variable);
        let shown = queried.host().unwrap_or_default();
        assert!(
            !shown.contains("abc123"),
            "a token in the query survived into what is shown"
        );
    }

    /// An endpoint that can be driven refuses nothing; one that cannot refuses both
    /// halves of the run at once, which is the whole reason the question is asked
    /// in one place.
    #[test]
    fn an_endpoint_nothing_can_drive_refuses_the_reads_and_the_writes_together() {
        assert!(Target::local().refusal().is_none());
        assert!(Target::at("ssh://nas.local", Origin::Variable)
            .refusal()
            .is_none());

        let beyond = Target {
            reach: Reach::Beyond("https://nas.local:2376".to_owned()),
            origin: Origin::Variable,
        };
        assert!(beyond.is_remote());
        assert!(matches!(
            beyond.refusal(),
            Some(super::Failure::Unsupported { .. })
        ));

        let missing = Target::missing("nas");
        assert!(missing.is_remote(), "not this machine's daemon either");
        assert_eq!(missing.endpoint(), None, "nothing may be handed to Compose");
        assert_eq!(missing.context(), Some("nas"));
        assert!(matches!(
            missing.refusal(),
            Some(super::Failure::NoSuchContext { ref name }) if name == "nas"
        ));
    }

    #[test]
    fn only_a_context_answers_for_which_context_chose_it() {
        assert_eq!(Target::at("tcp://a", Origin::Variable).context(), None);
        assert_eq!(
            Target::at("tcp://a", Origin::Context("nas".to_owned())).context(),
            Some("nas")
        );
    }

    /// Docker's own order, which is the part of this that can be wrong.
    #[test]
    fn the_variable_beats_the_environments_context_beats_the_recorded_one() {
        assert_eq!(
            chosen(Some("tcp://a"), Some("nas"), Some("shed")),
            Choice::Endpoint("tcp://a".to_owned())
        );
        assert_eq!(
            chosen(None, Some("nas"), Some("shed")),
            Choice::Named("nas".to_owned())
        );
        assert_eq!(
            chosen(None, None, Some("shed")),
            Choice::Named("shed".to_owned())
        );
        assert_eq!(chosen(None, None, None), Choice::Defaults);
    }

    /// An empty variable has not chosen anything, which is different from choosing.
    #[test]
    fn an_empty_variable_has_not_chosen_anything() {
        assert_eq!(
            chosen(Some(""), Some(" "), Some("shed")),
            Choice::Named("shed".to_owned())
        );
        assert_eq!(chosen(Some("  "), None, None), Choice::Defaults);
    }

    /// Asking for the local context ends the search rather than continuing it.
    ///
    /// Falling through would take an operator who explicitly said "this machine" and
    /// hand them whatever a configuration file still remembers, which is the one
    /// answer they have ruled out.
    #[test]
    fn asking_for_the_local_context_is_not_a_reason_to_read_the_file() {
        assert_eq!(
            chosen(None, Some("default"), Some("shed")),
            Choice::Defaults
        );
        assert_eq!(chosen(None, None, Some("default")), Choice::Defaults);
    }
}
