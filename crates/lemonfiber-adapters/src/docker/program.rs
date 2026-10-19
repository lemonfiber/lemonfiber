//! Running an image's own program once, in a container of its own, through the engine
//! API: created, attached, started, read, waited on and removed.

use std::collections::HashMap;
use std::time::SystemTime;

use bollard::container::LogOutput;
use bollard::models::{
    ContainerCreateBody, ContainerSummary, HostConfig, HostConfigLogConfig, Mount, MountType,
};
use bollard::query_parameters::{
    AttachContainerOptionsBuilder, ListContainersOptionsBuilder, RemoveContainerOptionsBuilder,
    WaitContainerOptions,
};
use bollard::Docker;
use tokio_stream::StreamExt as _;

use lemonfiber_ports::program::{Ended, Ran, Run, Unran, LEFT_BEHIND};

use super::{wording, Daemon};

/// The label every proof's container carries, and nothing else this build starts.
pub(super) const PROOF_LABEL: &str = "app.lemonfiber.proof";

/// The network a proof's container is held to: none.
const NO_NETWORK: &str = "none";

/// The logging driver a proof's container is given: none, so nothing it writes is kept.
const NO_LOGS: &str = "none";

/// Run `run` once, removing its container whatever it came to.
pub(super) async fn ran(daemon: &Daemon, run: &Run) -> Result<Ran, Unran> {
    let docker = daemon
        .client()
        .await
        .map_err(|failure| Unran::Unreachable {
            reason: failure.to_string(),
        })?;
    swept(docker).await;
    let created = docker
        .create_container(None, body(run))
        .await
        .map_err(|error| uncreated(&error, &run.image))?;
    let id = created.id;
    let outcome = watched(docker, &id, run).await;
    removed(docker, &id).await;
    outcome
}

/// What the engine is asked to create for `run`.
pub(super) fn body(run: &Run) -> ContainerCreateBody {
    ContainerCreateBody {
        image: Some(run.image.clone()),
        cmd: Some(run.args.clone()),
        env: Some(run.env.clone()),
        user: run.user.clone(),
        attach_stdin: Some(false),
        attach_stdout: Some(true),
        attach_stderr: Some(false),
        tty: Some(false),
        open_stdin: Some(false),
        network_disabled: Some(true),
        labels: Some(HashMap::from([(PROOF_LABEL.to_owned(), "run".to_owned())])),
        host_config: Some(HostConfig {
            network_mode: Some(NO_NETWORK.to_owned()),
            security_opt: Some(run.security_opt.clone()),
            cap_drop: Some(run.cap_drop.clone()),
            cap_add: Some(run.cap_add.clone()),
            pids_limit: Some(run.pids_limit),
            readonly_rootfs: Some(run.read_only),
            memory: run.memory,
            privileged: Some(false),
            log_config: Some(HostConfigLogConfig {
                typ: Some(NO_LOGS.to_owned()),
                config: None,
            }),
            mounts: Some(
                run.configuration
                    .iter()
                    .map(|configuration| Mount {
                        typ: Some(MountType::BIND),
                        source: Some(configuration.source.display().to_string()),
                        target: Some(configuration.target.clone()),
                        read_only: Some(false),
                        ..Mount::default()
                    })
                    .collect(),
            ),
            ..HostConfig::default()
        }),
        ..ContainerCreateBody::default()
    }
}

/// Why the engine would not create the container.
pub(super) fn uncreated(error: &bollard::errors::Error, image: &str) -> Unran {
    match error {
        bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        } => Unran::NoImage {
            image: image.to_owned(),
        },
        bollard::errors::Error::DockerResponseServerError { message, .. } => Unran::Refused {
            reason: message.clone(),
        },
        other => Unran::Unreachable {
            reason: wording(other),
        },
    }
}

/// Attach to the container, start it, and read it to its end or its limit.
async fn watched(docker: &Docker, id: &str, run: &Run) -> Result<Ran, Unran> {
    let attaching = AttachContainerOptionsBuilder::default()
        .stdout(true)
        .stderr(false)
        .stream(true)
        .logs(false)
        .build();
    let attached = docker
        .attach_container(id, Some(attaching))
        .await
        .map_err(|error| refused(&error))?;
    docker
        .start_container(id, None)
        .await
        .map_err(|error| refused(&error))?;
    let mut stdout = Vec::new();
    let ending = tokio::time::timeout(run.within, async {
        read(attached.output, &mut stdout, run.read_at_most).await;
        exited(docker, id).await
    })
    .await;
    match ending {
        Ok(exit) => exit.map(|code| Ran {
            ended: Ended::Exited(code),
            stdout,
        }),
        Err(_) => Ok(Ran {
            ended: Ended::Overran,
            stdout,
        }),
    }
}

/// Everything the container writes to standard output, keeping at most `most` bytes and
/// reading the rest away so it never waits on a reader that stopped.
pub(super) async fn read<S>(mut output: S, kept: &mut Vec<u8>, most: usize)
where
    S: tokio_stream::Stream<Item = Result<LogOutput, bollard::errors::Error>> + Unpin,
{
    while let Some(Ok(chunk)) = output.next().await {
        if let LogOutput::StdOut { message } = chunk {
            let room = most.saturating_sub(kept.len());
            kept.extend(message.iter().take(room));
        }
    }
}

/// The status the container exited with.
async fn exited(docker: &Docker, id: &str) -> Result<i64, Unran> {
    let mut waiting = docker.wait_container(id, None::<WaitContainerOptions>);
    match waiting.next().await {
        Some(waited) => status(waited),
        None => Err(Unran::Refused {
            reason: "the engine ended the wait without saying how the run ended".to_owned(),
        }),
    }
}

/// The exit status a wait answered, which the client reports as a failure where it is
/// not zero.
pub(super) fn status(
    waited: Result<bollard::models::ContainerWaitResponse, bollard::errors::Error>,
) -> Result<i64, Unran> {
    match waited {
        Ok(answer) => Ok(answer.status_code),
        Err(bollard::errors::Error::DockerContainerWaitError { code, .. }) => Ok(code),
        Err(other) => Err(refused(&other)),
    }
}

/// A refusal from an engine that has already answered once.
pub(super) fn refused(error: &bollard::errors::Error) -> Unran {
    Unran::Refused {
        reason: wording(error),
    }
}

/// Take the container off the engine with anything it made.
async fn removed(docker: &Docker, id: &str) {
    let options = RemoveContainerOptionsBuilder::default()
        .force(true)
        .v(true)
        .build();
    let _ = docker.remove_container(id, Some(options)).await;
}

/// Remove every proof's container a run before this one left behind.
pub(super) async fn swept(docker: &Docker) {
    let filters = HashMap::from([("label".to_owned(), vec![PROOF_LABEL.to_owned()])]);
    let options = ListContainersOptionsBuilder::default()
        .all(true)
        .filters(&filters)
        .build();
    let Ok(listed) = docker.list_containers(Some(options)).await else {
        return;
    };
    for id in left_behind(&listed, SystemTime::now()) {
        removed(docker, &id).await;
    }
}

/// Every proof's container created longer ago than any run lasts.
pub(super) fn left_behind(listed: &[ContainerSummary], now: SystemTime) -> Vec<String> {
    let Ok(since) = now.duration_since(SystemTime::UNIX_EPOCH) else {
        return Vec::new();
    };
    let before = i64::try_from(since.saturating_sub(LEFT_BEHIND).as_secs()).unwrap_or(i64::MAX);
    listed
        .iter()
        .filter(|container| {
            container
                .labels
                .as_ref()
                .is_some_and(|labels| labels.contains_key(PROOF_LABEL))
        })
        .filter(|container| container.created.is_some_and(|created| created < before))
        .filter_map(|container| container.id.clone())
        .collect()
}

#[cfg(test)]
mod tests;
