use super::{
    Container, Diagnose, ExecOutput, Failure, Health, Image, Lifecycle, LogLine, LogQuery,
    Published, Stats, Stream,
};

/// An image carries who is standing on it, because that and not its name is what
/// decides whether removing it takes something else with it.
#[test]
fn an_image_carries_the_projects_standing_on_it() {
    let image = Image {
        tags: vec!["linuxserver/sonarr:4.0.15".to_owned()],
        bytes: 512,
        projects: vec!["lemonfiber".to_owned(), String::new()],
    };
    assert_eq!(image.clone(), image);
    assert_eq!(image.projects.len(), 2);
    assert!(
        image.projects.iter().any(String::is_empty),
        "a container under no Compose project is reported as one: {image:?}"
    );
    assert_ne!(
        image,
        Image {
            projects: vec!["lemonfiber".to_owned()],
            ..image.clone()
        }
    );
}

#[test]
fn an_unreachable_engine_keeps_the_transport_s_own_words() {
    let problem = Failure::Unreachable {
        reason: "connection refused".to_owned(),
    }
    .problem();
    assert_eq!(problem.detail.as_deref(), Some("connection refused"));
    assert!(!problem.remedies.is_empty());
}

/// The three ways a remote engine refuses stay three answers.
///
/// Folded together they all read as the local Docker Desktop being stopped,
/// which is wrong for a name that does not resolve, wrong for a port nothing is
/// listening on, and wrong for a key the other machine will not take. Each is
/// checked for its own code, its own remedy, and for naming the host — the one
/// thing all three have in common and the one the operator most needs.
#[test]
fn each_way_of_failing_to_reach_a_remote_engine_keeps_its_own_answer() {
    let failures = [
        Failure::Unresolved {
            host: "ssh://nas.local".to_owned(),
            reason: "nodename nor servname provided".to_owned(),
        },
        Failure::Refused {
            host: "tcp://nas.local:2375".to_owned(),
            reason: "connection refused".to_owned(),
        },
        Failure::Rejected {
            host: "ssh://media@nas.local".to_owned(),
            reason: "Permission denied (publickey)".to_owned(),
        },
    ];

    let mut codes = Vec::new();
    for failure in &failures {
        let problem = failure.problem();
        let summary = problem.summary.clone();
        assert!(
            summary.contains("nas.local"),
            "the host is named: {summary}"
        );
        assert!(!problem.remedies.is_empty(), "{summary}");
        assert!(
            !problem
                .remedies
                .iter()
                .any(|remedy| remedy.action.contains("Docker Desktop")),
            "a remote failure is not a local Docker Desktop: {summary}"
        );
        assert!(failure.to_string().contains("nas.local"));
        codes.push(problem.code);
    }
    codes.dedup();
    assert_eq!(codes.len(), 3, "three conditions, three codes: {codes:?}");
    assert_ne!(codes.first(), Some(&super::ENGINE_UNREACHABLE));
}

/// A remote host that went quiet is its own answer, not the local engine's.
///
/// The catch-all used to be the local one, and its remedy is to start Docker on
/// the machine the operator is sitting at — which, for somebody whose laptop is
/// talking to a server, is running. This is the honest end of the list: it names
/// the host, quotes the transport, and sends nobody to the wrong machine.
#[test]
fn a_remote_host_that_went_quiet_does_not_borrow_the_local_engines_answer() {
    let problem = Failure::Unanswered {
        host: "ssh://media@nas.local".to_owned(),
        reason: "the stream ended unexpectedly".to_owned(),
    }
    .problem();

    assert_eq!(problem.code, super::HOST_SILENT);
    assert!(problem.summary.contains("nas.local"), "the host is named");
    assert_eq!(
        problem.detail.as_deref(),
        Some("the stream ended unexpectedly"),
        "and the transport keeps its own words"
    );
    assert!(
        !problem
            .remedies
            .iter()
            .any(|remedy| remedy.action.contains("Docker Desktop")),
        "nobody is sent to the machine they are already sitting at"
    );
}

/// A context this machine has no record of is refused, not quietly answered.
///
/// The fall back nothing here makes is the one the client library makes on its
/// own: a name it does not know becomes the local daemon. An operator who
/// mistyped the server would then be told about their laptop, in words that read
/// exactly like the answer they asked for.
#[test]
fn a_context_this_machine_has_no_record_of_is_refused_rather_than_answered() {
    let refusal = Failure::NoSuchContext {
        name: "nas".to_owned(),
    };

    assert!(
        refusal.to_string().contains("nas"),
        "the name that was asked for is the one thing worth saying back"
    );

    let problem = refusal.problem();
    assert_eq!(problem.code, super::UNKNOWN_CONTEXT);
    assert!(
        problem.meaning.contains("Nothing was read"),
        "the operator is told the machine is as they left it"
    );
    assert_eq!(
        problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.clone()),
        Some("docker context ls".to_owned()),
        "and given the command that lists the names it does have"
    );
}

/// An endpoint nothing here can drive is refused rather than half-driven.
///
/// Reading one machine and writing to another is the failure this whole seam
/// exists to prevent, so an endpoint the reads cannot use must not quietly
/// become one the writes do.
#[test]
fn an_endpoint_that_cannot_be_driven_is_refused_by_name() {
    let problem = Failure::Unsupported {
        endpoint: "https://nas.local:2376".to_owned(),
    }
    .problem();
    assert!(problem.summary.contains("https://nas.local:2376"));
    assert!(problem
        .remedies
        .iter()
        .any(|remedy| remedy.action.contains("DOCKER_HOST")));
    assert!(problem.meaning.contains("Nothing was read"));
}

#[test]
fn a_missing_container_is_named_and_is_not_an_error() {
    let problem = Failure::NoSuchContainer {
        name: "sonarr".to_owned(),
    }
    .problem();
    assert!(problem.summary.contains("sonarr"));
    assert!(!problem.remedies.is_empty());
    assert!(Failure::NoSuchContainer {
        name: "sonarr".to_owned()
    }
    .to_string()
    .contains("sonarr"));
}

#[test]
fn observations_carry_the_service_they_describe() {
    let container = Container {
        id: "abc123".to_owned(),
        published: vec![Published {
            address: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            port: 8989,
        }],
        project: "lemonfiber".to_owned(),
        service: "sonarr".to_owned(),
        lifecycle: Lifecycle::Running,
        health: Health::Healthy,
        mounts: Vec::new(),
        exit: None,
    };
    assert_eq!(container.clone(), container);
    assert_eq!(container.service, "sonarr");
    assert!(container
        .published
        .iter()
        .all(|published| published.address.is_loopback()));

    let line = LogLine {
        service: "sonarr".to_owned(),
        stream: Stream::Stderr,
        at: Some("2026-07-25T10:00:00Z".to_owned()),
        line: "something happened".to_owned(),
    };
    assert_eq!(line.clone().stream, Stream::Stderr);
    assert_eq!(line.at.as_deref(), Some("2026-07-25T10:00:00Z"));

    let stats = Stats {
        cpu: 0.5,
        memory_bytes: 1024,
    };
    assert!((stats.cpu - 0.5).abs() < f64::EPSILON);

    let exec = ExecOutput {
        status: Some(0),
        stdout: "203.0.113.7".to_owned(),
    };
    assert_eq!(exec.clone().stdout, "203.0.113.7");
}

#[test]
fn asking_for_recent_output_does_not_ask_to_keep_listening() {
    let recent = LogQuery::recent(50);
    assert_eq!(
        recent,
        LogQuery {
            tail: 50,
            follow: false
        }
    );
}

#[test]
fn every_lifecycle_and_health_value_is_distinguishable() {
    let lifecycles = [
        Lifecycle::Created,
        Lifecycle::Running,
        Lifecycle::Paused,
        Lifecycle::Restarting,
        Lifecycle::Exited,
        Lifecycle::Removing,
        Lifecycle::Dead,
    ];
    assert_eq!(lifecycles.len(), 7);
    assert_ne!(
        format!("{:?}", Lifecycle::Running),
        format!("{:?}", Lifecycle::Dead)
    );

    let healths = [
        Health::Starting,
        Health::Healthy,
        Health::Unhealthy,
        Health::None,
    ];
    assert_eq!(healths.len(), 4);
    assert_ne!(
        format!("{:?}", Health::Healthy),
        format!("{:?}", Health::None)
    );
}
