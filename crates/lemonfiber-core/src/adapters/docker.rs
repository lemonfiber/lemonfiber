//! Reaching the container engine's own API.
//!
//! Translation, and no decisions. What the engine reports becomes the port's
//! vocabulary here; what any of it *means* is decided in `crate::docker`, where
//! a test can drive it without a daemon.
//!
//! The client is built on first use rather than at construction, for two
//! reasons that turn out to be the same reason. The engine's API version has to
//! be agreed with the daemon before anything is asked of it, and that agreement
//! is itself a request — so a constructor that could not fail would have to
//! guess a version, and one that could fail would make an absent daemon a
//! startup error rather than the ordinary, recoverable condition it is. An
//! operator whose Docker Desktop is still starting runs `lemonfiber config
//! show` and expects an answer.
//!
//! Streams become channels here. See `.docs/architecture/ports-and-adapters.md`
//! for why the port says `Receiver`, and `.docs/architecture/engine-api.md` for
//! what the engine is asked and how the answers are tested.

use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;
use bollard::models::ContainerSummary;
use bollard::query_parameters::{
    ListContainersOptionsBuilder, LogsOptionsBuilder, StatsOptionsBuilder,
};
use bollard::Docker;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::OnceCell;
use tokio_stream::StreamExt as _;

use crate::ports::docker::{
    Container, Engine, ExecOutput, Failure, Lifecycle, LogLine, LogQuery, Stats, Stream,
};

mod translate;

use translate::{describe, sampled, split_timestamp};

/// How many observations may queue before the producer waits.
///
/// A reader that has stopped reading should slow its producer down rather than
/// grow a buffer without limit: the alternative to a bounded channel is a log
/// viewer that consumes memory for as long as it is scrolled back.
const BACKLOG: usize = 256;

/// The label Compose puts on every container it creates, naming the project.
const PROJECT_LABEL: &str = "com.docker.compose.project";

/// The label naming the service a container implements.
const SERVICE_LABEL: &str = "com.docker.compose.service";

/// Where the engine is listening.
#[derive(Debug)]
enum Address {
    /// Wherever this machine's conventions say, honouring `DOCKER_HOST`.
    Local,
    /// A named socket, which is how a test points this at something it controls.
    Socket(String),
}

/// The container engine on this machine, reached over its own API.
#[derive(Debug)]
pub struct Daemon {
    address: Address,
    client: OnceCell<Docker>,
}

impl Daemon {
    /// The engine this machine is configured to talk to.
    #[must_use]
    pub fn local() -> Self {
        Self {
            address: Address::Local,
            client: OnceCell::new(),
        }
    }

    /// The engine listening on a particular socket.
    ///
    /// This is the seam a remote context will arrive through, and the one tests
    /// use to point the adapter at an engine they wrote themselves.
    #[must_use]
    pub fn at(socket: &Path) -> Self {
        Self {
            address: Address::Socket(socket.display().to_string()),
            client: OnceCell::new(),
        }
    }

    /// The connected client, built and version-matched on first use.
    ///
    /// A failure is not remembered. The daemon being down is a condition that
    /// ends, and an adapter that cached the first refusal would keep reporting
    /// it long after Docker Desktop finished starting.
    async fn client(&self) -> Result<&Docker, Failure> {
        self.client
            .get_or_try_init(|| async {
                let connected = match &self.address {
                    Address::Local => Docker::connect_with_local_defaults(),
                    Address::Socket(path) => {
                        Docker::connect_with_unix(path, TIMEOUT, bollard::API_DEFAULT_VERSION)
                    }
                };
                let docker = connected.map_err(unreachable)?;

                // Asking the daemon its version settles two things for one
                // request: which API generation is in play, and whether there
                // is a daemon at all — so an absent engine is reported as
                // absent rather than as whatever the first real call failed to
                // decode. See the client library's note in
                // `.docs/architecture/engine-api.md` on what it does with the
                // answer today.
                docker.negotiate_version().await.map_err(unreachable)
            })
            .await
    }

    /// Every container Compose created for this project, running or not.
    ///
    /// Filtered by label at the engine rather than here, so a machine running
    /// several stacks does not send nineteen containers over the socket to have
    /// eighteen of them discarded.
    async fn containers(&self, project: &str) -> Result<Vec<ContainerSummary>, Failure> {
        let mut filters = HashMap::new();
        filters.insert(
            "label".to_owned(),
            vec![format!("{PROJECT_LABEL}={project}")],
        );

        let options = ListContainersOptionsBuilder::default()
            .all(true)
            .filters(&filters)
            .build();

        self.client()
            .await?
            .list_containers(Some(options))
            .await
            .map_err(unreachable)
    }
}

/// How long a request may take before the engine counts as unreachable.
const TIMEOUT: u64 = 30;

/// Any refusal from the engine, in the engine's own words.
///
/// Every way it can decline arrives here, because from the operator's side
/// there is one question — can lemonfiber see Docker — and the detail belongs
/// in the detail field rather than in a taxonomy nobody acts on differently.
///
/// A daemon that answered keeps its own message. Its wording is written for
/// someone holding a terminal, and wrapping it in a transport description only
/// buries the sentence worth reading.
fn unreachable(error: bollard::errors::Error) -> Failure {
    let reason = match error {
        bollard::errors::Error::DockerResponseServerError { message, .. } => message,
        answered_nothing => answered_nothing.to_string(),
    };
    Failure::Unreachable { reason }
}

#[async_trait]
impl Engine for Daemon {
    async fn list(&self, project: &str) -> Result<Vec<Container>, Failure> {
        Ok(self
            .containers(project)
            .await?
            .into_iter()
            .map(describe)
            .collect())
    }

    async fn exec(&self, container: &str, argv: &[String]) -> Result<ExecOutput, Failure> {
        let docker = self.client().await?;

        let config = bollard::models::ExecConfig {
            cmd: Some(argv.to_vec()),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        // A container that is not there is the one refusal an operator can act
        // on differently, so it keeps its own variant all the way up.
        let created = match docker.create_exec(container, config).await {
            Ok(created) => created,
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => {
                return Err(Failure::NoSuchContainer {
                    name: container.to_owned(),
                })
            }
            Err(error) => return Err(unreachable(error)),
        };

        let started = docker
            .start_exec(&created.id, None)
            .await
            .map_err(unreachable)?;

        let mut stdout = String::new();
        if let bollard::exec::StartExecResults::Attached { mut output, .. } = started {
            while let Some(chunk) = output.next().await {
                stdout.push_str(&chunk.map_err(unreachable)?.to_string());
            }
        }

        let inspected = docker
            .inspect_exec(&created.id)
            .await
            .map_err(unreachable)?;

        Ok(ExecOutput {
            status: inspected
                .exit_code
                .and_then(|code| i32::try_from(code).ok()),
            stdout,
        })
    }

    async fn stats(&self, project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
        let containers = self.containers(project).await?;
        let docker = self.client().await?.clone();
        let (sender, receiver) = channel(BACKLOG);

        // A container that is not running is not using anything, and asking it
        // how busy it is would be one open stream per stopped service.
        for described in containers.into_iter().map(describe) {
            if described.lifecycle == Lifecycle::Running {
                tokio::spawn(sample_into(docker.clone(), described, sender.clone()));
            }
        }

        Ok(receiver)
    }

    async fn logs(
        &self,
        project: &str,
        services: &[String],
        query: LogQuery,
    ) -> Result<Receiver<LogLine>, Failure> {
        let containers = self.containers(project).await?;
        let docker = self.client().await?.clone();
        let (sender, receiver) = channel(BACKLOG);

        // Stopped services are included: their scrollback is usually the reason
        // the operator opened the log viewer at all.
        for described in containers.into_iter().map(describe) {
            if services.is_empty() || services.contains(&described.service) {
                tokio::spawn(read_into(docker.clone(), described, query, sender.clone()));
            }
        }

        Ok(receiver)
    }
}

/// Sample one container until the reader goes away.
///
/// One task per container rather than one loop over all of them: a service
/// whose stream stalls then delays its own panel and nothing else.
async fn sample_into(docker: Docker, container: Container, sender: Sender<(String, Stats)>) {
    let options = StatsOptionsBuilder::default().stream(true).build();
    let mut samples = std::pin::pin!(docker.stats(&container.id, Some(options)));

    // A closed channel is the reader having moved on, which is ordinary — the
    // panel was closed. Stopping quietly is the whole handling it needs.
    while let Some(Ok(sample)) = samples.next().await {
        let observed = (container.service.clone(), sampled(&sample));
        if sender.send(observed).await.is_err() {
            break;
        }
    }
}

/// Read one container's output until it ends or the reader goes away.
async fn read_into(docker: Docker, container: Container, query: LogQuery, sender: Sender<LogLine>) {
    let options = LogsOptionsBuilder::default()
        .stdout(true)
        .stderr(true)
        .timestamps(true)
        .follow(query.follow)
        .tail(&query.tail.to_string())
        .build();

    let mut chunks = std::pin::pin!(docker.logs(&container.id, Some(options)));
    while let Some(Ok(chunk)) = chunks.next().await {
        let stream = match &chunk {
            bollard::container::LogOutput::StdErr { .. } => Stream::Stderr,
            _ => Stream::Stdout,
        };

        // One engine chunk is not one line. Splitting here rather than at the
        // reader means every consumer gets lines, and none of them reimplements
        // the splitting.
        let text = chunk.to_string();
        for line in text.lines() {
            let (at, line) = split_timestamp(line);
            let sending = LogLine {
                service: container.service.clone(),
                stream,
                at,
                line,
            };
            if sender.send(sending).await.is_err() {
                return;
            }
        }
    }
}
