use super::{beneath, endpoint_of, named_context, resolved, unverifiable, BENEATH_HOME};
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
        let root = lemonfiber_fixtures::scratch::Scratch::named(&format!("context-{label}")).kept();
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

    /// Record a directory with nothing readable in it, as a half-removed context
    /// leaves behind.
    fn empty(&self, digest: &str) -> &Self {
        let _ = std::fs::create_dir_all(self.0.join("contexts").join("meta").join(digest));
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
///
/// A directory holding nothing readable is put beside the good one on purpose.
/// Docker leaves those behind — a context removed mid-write, a half-created one —
/// and a reader that stopped at the first unreadable entry would report a context
/// this machine does have as one it does not.
#[test]
fn a_context_this_machine_does_not_have_is_not_the_local_daemon() {
    let recorded = Recorded::new("missing");
    recorded
        .empty("000")
        .context("aaa", "nas", "ssh://media@nas.local");

    let target = resolved(None, Some("typo"), false, Some(recorded.path()));
    assert_eq!(target.reach, Reach::Missing("typo".to_owned()));
    assert!(target.refusal().is_some(), "{target:?}");
    assert_eq!(target.endpoint(), None, "nothing is handed to Compose");

    let found = resolved(None, Some("nas"), false, Some(recorded.path()));
    assert_eq!(
        found.endpoint(),
        Some("ssh://media@nas.local"),
        "the unreadable neighbour did not stop the search"
    );
}

/// Where Docker keeps its configuration, both ways round.
///
/// The named directory wins outright: an operator who moved it did so because the
/// conventional place is wrong for them, and reading the conventional place
/// anyway would answer with contexts they have stopped using.
#[test]
fn a_named_configuration_directory_beats_the_one_beneath_the_home() {
    assert_eq!(
        beneath(
            Some("/srv/docker".to_owned()),
            Some("/home/media".to_owned())
        ),
        Some(PathBuf::from("/srv/docker"))
    );
    assert_eq!(
        beneath(None, Some("/home/media".to_owned())),
        Some(PathBuf::from("/home/media").join(BENEATH_HOME))
    );
    assert_eq!(beneath(None, None), None, "a machine that says neither");
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
