use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use bollard::container::LogOutput;
use bollard::models::{ContainerSummary, ContainerWaitResponse, MountType};
use lemonfiber_ports::program::{Configuration, Joined, Run, Unran};

use lemonfiber_ports::program::LEFT_BEHIND;

use super::{body, left_behind, read, status, uncreated, unstarted, PROOF_LABEL, STARTED_WITHIN};

fn run() -> Run {
    Run {
        image: "ghcr.io/recyclarr/recyclarr@sha256:9c6c".to_owned(),
        args: vec!["--version".to_owned()],
        env: vec!["TZ=UTC".to_owned()],
        user: Some("1000:1000".to_owned()),
        security_opt: vec!["no-new-privileges:true".to_owned()],
        cap_drop: vec!["ALL".to_owned()],
        cap_add: Vec::new(),
        pids_limit: 1024,
        read_only: true,
        memory: Some(256 * 1024 * 1024),
        configuration: Some(Configuration {
            source: PathBuf::from("/tmp/copy"),
            target: "/config".to_owned(),
        }),
        within: Duration::from_secs(30),
        read_at_most: 8,
        network: Joined::Nothing,
        pull: false,
    }
}

#[test]
fn a_run_reading_the_hosts_addresses_joins_the_hosts_network_and_nothing_else_changes() {
    let host = Run {
        network: Joined::Host,
        ..run()
    };
    let asked = body(&host);
    assert_eq!(asked.network_disabled, Some(false));
    let config = asked.host_config.unwrap_or_default();
    assert_eq!(config.network_mode.as_deref(), Some("host"));
    assert_eq!(config.cap_drop, Some(vec!["ALL".to_owned()]));
    assert_eq!(config.readonly_rootfs, Some(true));
}

#[test]
fn a_proofs_container_has_no_network_no_shell_and_only_what_the_run_settled() {
    let asked = body(&run());
    assert_eq!(
        asked.image.as_deref(),
        Some("ghcr.io/recyclarr/recyclarr@sha256:9c6c")
    );
    assert_eq!(asked.cmd, Some(vec!["--version".to_owned()]));
    assert_eq!(asked.entrypoint, None);
    assert_eq!(asked.env, Some(vec!["TZ=UTC".to_owned()]));
    assert_eq!(asked.user.as_deref(), Some("1000:1000"));
    assert_eq!(asked.network_disabled, Some(true));
    assert_eq!(asked.tty, Some(false));
    assert_eq!(asked.open_stdin, Some(false));
    assert_eq!(asked.attach_stdout, Some(true));
    assert_eq!(asked.attach_stderr, Some(false));
    assert_eq!(
        asked.labels,
        Some(HashMap::from([(PROOF_LABEL.to_owned(), "run".to_owned())]))
    );
    assert!(asked
        .labels
        .iter()
        .flatten()
        .all(|(name, _)| !name.starts_with("com.docker.compose")));
    let host = asked.host_config.unwrap_or_default();
    assert_eq!(host.network_mode.as_deref(), Some("none"));
    assert_eq!(host.cap_drop, Some(vec!["ALL".to_owned()]));
    assert_eq!(host.cap_add, Some(Vec::new()));
    assert_eq!(
        host.security_opt,
        Some(vec!["no-new-privileges:true".to_owned()])
    );
    assert_eq!(host.pids_limit, Some(1024));
    assert_eq!(host.readonly_rootfs, Some(true));
    assert_eq!(host.memory, Some(256 * 1024 * 1024));
    assert_eq!(host.privileged, Some(false));
    assert_eq!(
        host.log_config.and_then(|logs| logs.typ).as_deref(),
        Some("none")
    );
    assert_eq!(host.binds, None);
    assert_eq!(host.devices, None);
    let mounts = host.mounts.unwrap_or_default();
    assert_eq!(mounts.len(), 1);
    let mount = mounts.into_iter().next().unwrap_or_default();
    assert_eq!(mount.typ, Some(MountType::BIND));
    assert_eq!(mount.source.as_deref(), Some("/tmp/copy"));
    assert_eq!(mount.target.as_deref(), Some("/config"));
    assert_eq!(mount.bind_options, None);
}

#[test]
fn a_run_with_no_directory_mounts_nothing() {
    let bare = Run {
        configuration: None,
        user: None,
        memory: None,
        ..run()
    };
    let asked = body(&bare);
    assert_eq!(asked.user, None);
    let host = asked.host_config.unwrap_or_default();
    assert_eq!(host.mounts, Some(Vec::new()));
    assert_eq!(host.memory, None);
}

fn refusal(status_code: u16, message: &str) -> bollard::errors::Error {
    bollard::errors::Error::DockerResponseServerError {
        status_code,
        message: message.to_owned(),
    }
}

#[test]
fn an_image_the_engine_does_not_hold_is_named_and_never_pulled() {
    assert_eq!(
        uncreated(&refusal(404, "No such image"), "a@sha256:b"),
        Unran::NoImage {
            image: "a@sha256:b".to_owned()
        }
    );
    assert_eq!(
        uncreated(&refusal(400, "bind source path does not exist"), "a"),
        Unran::Refused {
            reason: "bind source path does not exist".to_owned()
        }
    );
    assert!(matches!(
        uncreated(&bollard::errors::Error::RequestTimeoutError, "a"),
        Unran::Unreachable { .. }
    ));
}

#[test]
fn how_a_run_ended_is_read_whether_the_client_calls_it_a_failure_or_not() {
    assert_eq!(
        status(Ok(ContainerWaitResponse {
            status_code: 0,
            error: None
        })),
        Ok(0)
    );
    assert_eq!(
        status(Err(bollard::errors::Error::DockerContainerWaitError {
            error: String::new(),
            code: 2
        })),
        Ok(2)
    );
    assert!(matches!(
        status(Err(refusal(500, "gone"))),
        Err(Unran::Refused { reason }) if reason == "gone"
    ));
}

fn stdout(text: &str) -> LogOutput {
    LogOutput::StdOut {
        message: text.as_bytes().to_vec().into(),
    }
}

#[tokio::test]
async fn what_is_read_stops_at_the_bound_and_never_takes_standard_error() {
    let chunks = tokio_stream::iter(
        [
            stdout("v8.7"),
            LogOutput::StdErr {
                message: b"warning".to_vec().into(),
            },
            stdout(".3 (Branch)"),
            stdout("more"),
        ]
        .map(Ok::<_, bollard::errors::Error>),
    );
    let mut kept = Vec::new();
    read(chunks, &mut kept, 8).await;
    assert_eq!(kept, b"v8.7.3 (".to_vec());
}

fn summary(id: &str, created: i64, labelled: bool) -> ContainerSummary {
    let label = if labelled {
        PROOF_LABEL
    } else {
        "com.docker.compose.project"
    };
    ContainerSummary {
        id: Some(id.to_owned()),
        created: Some(created),
        labels: Some(HashMap::from([(label.to_owned(), "run".to_owned())])),
        ..ContainerSummary::default()
    }
}

#[test]
fn only_a_proofs_container_older_than_any_run_is_taken_for_one_left_behind() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
    let old = 10_000 - i64::try_from(LEFT_BEHIND.as_secs()).unwrap_or_default() - 1;
    let edge = 10_000 - i64::try_from(LEFT_BEHIND.as_secs()).unwrap_or_default();
    let listed = vec![
        summary("old", old, true),
        summary("running", 9_990, true),
        summary("edge", edge, true),
        summary("theirs", old, false),
    ];
    assert_eq!(left_behind(&listed, now), vec!["old".to_owned()]);
    assert_eq!(
        left_behind(&listed, SystemTime::UNIX_EPOCH - Duration::from_secs(1)),
        Vec::<String>::new()
    );
}

#[test]
fn a_start_the_engine_did_not_finish_in_time_is_said_as_one_and_not_as_a_refusal() {
    assert_eq!(
        unstarted(bollard::errors::Error::RequestTimeoutError),
        Unran::Unstarted {
            seconds: STARTED_WITHIN.as_secs()
        }
    );
    assert_eq!(
        Unran::Unstarted { seconds: 120 }.to_string(),
        "the container engine did not start the container within 120 seconds"
    );
    assert!(matches!(
        unstarted(bollard::errors::Error::DockerResponseServerError {
            status_code: 500,
            message: "no such network".to_owned(),
        }),
        Unran::Refused { reason } if reason.contains("no such network")
    ));
}
