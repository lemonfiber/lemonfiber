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
