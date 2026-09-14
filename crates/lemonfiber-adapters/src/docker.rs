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
    ListContainersOptionsBuilder, ListImagesOptions, LogsOptionsBuilder, StatsOptionsBuilder,
};
use bollard::Docker;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::OnceCell;
use tokio_stream::StreamExt as _;

use lemonfiber_ports::docker::{
    Container, Engine, ExecOutput, Failure, Image, Images, Lifecycle, LogLine, LogQuery, Reach,
    Stats, Stream, Target,
};

pub mod context;
mod images;
mod refusal;
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

/// The container engine this run operates, reached over its own API.
///
/// Built from the target rather than from the environment, and that is the whole
/// point of it: the same value builds the Compose invocation, so the reads and the
/// writes cannot be aimed at different machines. An adapter that read `DOCKER_HOST`
/// for itself would be a second opinion about which machine this is, and the two
/// only disagree where it matters most — a remote context, where the reads came
/// from the laptop and the writes went to the server.
#[derive(Debug)]
pub struct Daemon {
    target: Target,
    client: OnceCell<Docker>,
}

impl Daemon {
    /// The engine this machine is configured to talk to.
    #[must_use]
    pub fn local() -> Self {
        Self::reaching(Target::local())
    }

    /// The engine this run is pointed at.
    ///
    /// The seam a remote context arrives through. Taking the resolved target rather
    /// than resolving one here is what keeps a single answer for the whole run.
    #[must_use]
    pub fn reaching(target: Target) -> Self {
        Self {
            target,
            client: OnceCell::new(),
        }
    }

    /// The engine listening on a particular socket.
    ///
    /// How a test points the adapter at an engine it wrote itself.
    #[must_use]
    pub fn at(socket: &Path) -> Self {
        Self::reaching(Target::socket(&socket.display().to_string()))
    }

    /// What a refusal from this engine means, given which engine it is.
    ///
    /// A local daemon has one way of being unavailable and a remote one has four,
    /// so where the engine is decides what its silence means. Asked of the daemon
    /// rather than of a free function because the target is the half of the question
    /// the error does not carry.
    fn refused(&self, error: &bollard::errors::Error) -> Failure {
        refusal::classify(&self.target, &wording(error))
    }

    /// The connected client, built and version-matched on first use.
    ///
    /// A failure is not remembered. The daemon being down is a condition that
    /// ends, and an adapter that cached the first refusal would keep reporting
    /// it long after Docker Desktop finished starting.
    async fn client(&self) -> Result<&Docker, Failure> {
        let target = &self.target;
        self.client
            .get_or_try_init(|| async {
                let docker = connect(target)?;

                // Asking the daemon its version settles two things for one
                // request: which API generation is in play, and whether there
                // is a daemon at all — so an absent engine is reported as
                // absent rather than as whatever the first real call failed to
                // decode. See the client library's note in
                // `.docs/architecture/engine-api.md` on what it does with the
                // answer today.
                docker
                    .negotiate_version()
                    .await
                    .map_err(|error| refusal::classify(target, &wording(&error)))
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
            .map_err(|error| self.refused(&error))
    }

    /// Every container on this machine, whatever project it belongs to and whether
    /// or not it belongs to one.
    ///
    /// Unfiltered, unlike [`Self::containers`], because the question it answers is
    /// about the machine rather than about this stack: an image is shared exactly
    /// when something outside the project is built on it, and a list narrowed to the
    /// project could never report that.
    ///
    /// Handed the client rather than asking for one. Its only caller is already
    /// holding it, and the connection is settled once and kept — so asking again
    /// could only ever succeed, which would leave a refusal here that nothing can
    /// reach and therefore nothing has ever checked.
    async fn every_container(&self, docker: &Docker) -> Result<Vec<ContainerSummary>, Failure> {
        let options = ListContainersOptionsBuilder::default().all(true).build();

        docker
            .list_containers(Some(options))
            .await
            .map_err(|error| self.refused(&error))
    }
}

#[async_trait]
impl Images for Daemon {
    async fn images(&self) -> Result<Vec<Image>, Failure> {
        let docker = self.client().await?;
        let listed = docker
            .list_images(None::<ListImagesOptions>)
            .await
            .map_err(|error| self.refused(&error))?;

        Ok(images::correlate(
            listed,
            &self.every_container(docker).await?,
        ))
    }
}

/// How long a request may take before the engine counts as unreachable.
const TIMEOUT: u64 = 30;

/// Build a client for wherever this run's engine is.
///
/// One arm per transport, and the endpoint handed over whole: the client library
/// parses it, and a second parser here would be a second opinion about somebody
/// else's address.
///
/// An endpoint nothing here can drive is refused before a connection is attempted,
/// which is the property the whole target exists for. Reaching for it anyway and
/// failing would leave the Compose half of the run still pointed at the same place
/// and perfectly able to change it.
fn connect(target: &Target) -> Result<Docker, Failure> {
    if let Some(refusal) = target.refusal() {
        return Err(refusal);
    }
    let built = match &target.reach {
        Reach::Socket(endpoint) => {
            Docker::connect_with_unix(endpoint, TIMEOUT, bollard::API_DEFAULT_VERSION)
        }
        Reach::Tcp(endpoint) => {
            Docker::connect_with_http(endpoint, TIMEOUT, bollard::API_DEFAULT_VERSION)
        }
        Reach::Ssh(endpoint) => {
            Docker::connect_with_ssh(endpoint, TIMEOUT, bollard::API_DEFAULT_VERSION, None)
        }
        // This machine's own conventions, which is what is left once the refusal
        // above has taken the two endpoints that have no transport.
        Reach::Local | Reach::Beyond(_) | Reach::Missing(_) => {
            Docker::connect_with_local_defaults()
        }
    };
    built.map_err(|error| refusal::classify(target, &wording(&error)))
}

/// Everything the transport said about a failure, causes included.
///
/// A daemon that answered keeps its own message: its wording is written for
/// somebody holding a terminal, and wrapping it in a transport description only
/// buries the sentence worth reading.
///
/// Everything else is read through its whole chain of causes, because that is where
/// the condition actually is. A connection failure arrives as one outer variant
/// whatever went wrong, and only the sentence at the bottom says whether a name went
/// nowhere, a port declined, or a key was refused.
fn wording(error: &bollard::errors::Error) -> String {
    if let bollard::errors::Error::DockerResponseServerError { message, .. } = error {
        return message.clone();
    }
    chained(error)
}

/// An error and everything under it, in order, without repeating itself.
///
/// Written over the standard trait rather than over the client library's own type,
/// which is what lets every shape of chain be driven here: one that quotes its cause
/// verbatim — the transparent wrappers do, and appending it again would say the same
/// sentence twice — and one that does not, which is where the condition finally
/// appears. A real chain of the second kind takes a socket to produce, so the thing
/// that decides is separated from the thing that needs one.
fn chained(error: &dyn std::error::Error) -> String {
    let mut said = error.to_string();
    let mut cause = error.source();
    while let Some(under) = cause {
        let next = under.to_string();
        if !said.contains(&next) {
            said.push_str(": ");
            said.push_str(&next);
        }
        cause = under.source();
    }
    said
}

/// Any refusal from a daemon that has already answered once.
///
/// The engine being there and declining is one question however far away it is, so
/// this stays the one answer for it. Which machine it was is decided at connection,
/// where the distinctions are, and a mid-stream chunk that stops arriving is not
/// evidence about a host name.
fn unreachable(error: &bollard::errors::Error) -> Failure {
    Failure::Unreachable {
        reason: wording(error),
    }
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
        executed(self, container, argv).await
    }

    async fn stats(&self, project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
        sampling(self, project).await
    }

    async fn logs(
        &self,
        project: &str,
        services: &[String],
        query: LogQuery,
    ) -> Result<Receiver<LogLine>, Failure> {
        read(self, project, services, query).await
    }
}

/// Run one command inside a container and collect what it said.
async fn executed(
    daemon: &Daemon,
    container: &str,
    argv: &[String],
) -> Result<ExecOutput, Failure> {
    let docker = daemon.client().await?;

    let config = bollard::models::ExecConfig {
        cmd: Some(argv.to_vec()),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        ..Default::default()
    };

    let created = docker
        .create_exec(container, config)
        .await
        .map_err(|error| refused_exec(error, container))?;

    let started = docker
        .start_exec(&created.id, None)
        .await
        .map_err(|error| daemon.refused(&error))?;

    let stdout = spoken(started).await;

    let inspected = docker
        .inspect_exec(&created.id)
        .await
        .map_err(|error| daemon.refused(&error))?;

    Ok(ExecOutput {
        status: inspected
            .exit_code
            .and_then(|code| i32::try_from(code).ok()),
        stdout,
    })
}

/// What a refusal to start an exec means.
///
/// A container that is not there is the one refusal an operator can act on differently,
/// so it keeps its own variant all the way up. Its own function because the other arm
/// needs a daemon that answers badly, which no test here has — handed the error
/// directly, both arms are ordinary.
fn refused_exec(error: bollard::errors::Error, container: &str) -> Failure {
    match error {
        bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        } => Failure::NoSuchContainer {
            name: container.to_owned(),
        },
        other => unreachable(&other),
    }
}

/// Everything an attached exec wrote, and nothing at all where it was not attached.
async fn spoken(started: bollard::exec::StartExecResults) -> String {
    match started {
        bollard::exec::StartExecResults::Attached { output, .. } => gathered(output).await,
        bollard::exec::StartExecResults::Detached => String::new(),
    }
}

/// Everything a stream of exec output said, joined in the order it arrived.
///
/// Generic over the stream, which is worth knowing about when reading its tests: a
/// generic is compiled once per type it is reached with, so a test handing it a
/// stream of its own would exercise a copy of this loop that no run ever executes
/// while the copy the product uses stayed unmeasured. The tests reach it through
/// [`spoken`], the way an exec does, so there is one copy and it is the one measured.
///
/// What arrived, and never a refusal. A stream that stops part-way is not this
/// function's to judge: the exec is asked for its exit status immediately afterwards,
/// and the two things that cut an output stream are both answered there — a
/// connection that is gone fails the inspection with it, and a container that died
/// mid-write reports the status it died with. A refusal raised here instead would be
/// a second way to say the same thing, reachable only through a transport failing
/// between two requests on one settled connection, which is to say reachable by
/// nothing that could be written down.
async fn gathered<S>(mut output: S) -> String
where
    S: tokio_stream::Stream<Item = Result<bollard::container::LogOutput, bollard::errors::Error>>
        + Unpin,
{
    let mut said = String::new();
    while let Some(Ok(chunk)) = output.next().await {
        said.push_str(&chunk.to_string());
    }
    said
}

/// One sampling task per running container, feeding one channel.
async fn sampling(daemon: &Daemon, project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
    // The connection first, and what to ask it about second. The order is the whole
    // of what makes both refusals reachable: the connection is settled once and kept,
    // so reaching for it *after* a listing has already succeeded is a refusal nothing
    // can produce — and one nothing can produce is one nothing has checked.
    let docker = daemon.client().await?.clone();
    let containers = daemon.containers(project).await?;
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

/// One reading task per named container, feeding one channel.
async fn read(
    daemon: &Daemon,
    project: &str,
    services: &[String],
    query: LogQuery,
) -> Result<Receiver<LogLine>, Failure> {
    // The connection first, for the reason the sampling beside this one takes them in
    // that order.
    let docker = daemon.client().await?.clone();
    let containers = daemon.containers(project).await?;
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

#[cfg(test)]
mod tests {
    use super::{chained, connect, refused_exec, spoken, Failure};
    use lemonfiber_ports::docker::{Origin, Target};

    /// An error with an optional cause, for driving a chain without a socket.
    ///
    /// The chains that matter here come out of a client library several layers deep,
    /// and producing a real one takes a real connection that fails a real way. What
    /// is actually being decided — whether a cause says something the sentence above
    /// it did not — needs neither, so it is driven over the standard trait instead.
    #[derive(Debug)]
    struct Said {
        text: String,
        under: Option<Box<Said>>,
    }

    impl Said {
        fn new(text: &str) -> Self {
            Self {
                text: text.to_owned(),
                under: None,
            }
        }

        fn caused_by(mut self, under: Self) -> Self {
            self.under = Some(Box::new(under));
            self
        }
    }

    impl std::fmt::Display for Said {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.text)
        }
    }

    impl std::error::Error for Said {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.under
                .as_deref()
                .map(|under| under as &(dyn std::error::Error + 'static))
        }
    }

    /// The condition is at the bottom of the chain, so the chain is what is read.
    ///
    /// Both shapes are here because both are ordinary. A transparent wrapper renders
    /// exactly what it wraps, and appending that again would say one sentence twice;
    /// a wrapper with its own words hides the condition until the cause under it is
    /// reached, which is the whole reason this walks rather than reading the top.
    #[test]
    fn what_a_failure_said_carries_its_causes_and_says_nothing_twice() {
        let quoting = Said::new("client error: connection refused")
            .caused_by(Said::new("connection refused"));
        assert_eq!(chained(&quoting), "client error: connection refused");

        let its_own_words = Said::new("error trying to connect")
            .caused_by(Said::new("tcp connect error").caused_by(Said::new("Connection refused")));
        assert_eq!(
            chained(&its_own_words),
            "error trying to connect: tcp connect error: Connection refused"
        );

        assert_eq!(chained(&Said::new("nothing under it")), "nothing under it");
    }

    /// A client is built for every transport this build can drive, and for none of
    /// the endpoints it cannot.
    ///
    /// Building one connects to nothing, which is what lets both remote transports be
    /// exercised here with no server on either. What the refused half proves is the
    /// property the target exists for: an endpoint the reads cannot use is turned
    /// away before a client exists, so it can never become one the writes use.
    #[test]
    fn a_client_is_built_for_each_transport_and_for_no_endpoint_beyond_them() {
        for endpoint in ["tcp://127.0.0.1:2375", "ssh://media@nas.local"] {
            let built = connect(&Target::at(endpoint, Origin::Variable)).is_ok();
            assert!(built, "{endpoint}");
        }

        for beyond in [
            Target::missing("nas"),
            Target::at("https://nas.local:2376", Origin::Variable),
        ] {
            let turned_away = connect(&beyond).is_err();
            assert!(turned_away, "{beyond:?}");
        }
    }

    /// Both refusals, one of which needs a daemon that answers badly.
    ///
    /// The 404 is the only one an operator can act on differently, and it is the one a
    /// real run produces. Everything else is the daemon being unreachable in some way,
    /// which is reachable here and nowhere else.
    #[test]
    fn a_container_that_is_not_there_is_told_apart_from_a_daemon_that_is_not_well() {
        let missing = refused_exec(
            bollard::errors::Error::DockerResponseServerError {
                status_code: 404,
                message: "no such container".to_owned(),
            },
            "sonarr",
        );
        assert!(
            matches!(missing, Failure::NoSuchContainer { ref name } if name == "sonarr"),
            "got: {missing:?}"
        );

        let otherwise = refused_exec(
            bollard::errors::Error::DockerResponseServerError {
                status_code: 500,
                message: "it is not well".to_owned(),
            },
            "sonarr",
        );
        assert!(
            !matches!(otherwise, Failure::NoSuchContainer { .. }),
            "a daemon fault is not a missing container: {otherwise:?}"
        );
    }

    /// An exec nobody attached to wrote nothing, which is not the same as an error.
    #[tokio::test]
    async fn an_exec_that_was_not_attached_to_says_nothing() {
        let said = spoken(bollard::exec::StartExecResults::Detached).await;
        assert_eq!(said, String::new());
    }

    /// What an exec attached to a stream of these chunks came to.
    ///
    /// Through the exec rather than through the loop underneath it, and that is the
    /// point rather than a convenience: the loop is generic over the stream it reads,
    /// so handing it a stream written here would compile a second copy of it and
    /// measure that one instead of the one a run uses.
    async fn attached(
        chunks: Vec<Result<bollard::container::LogOutput, bollard::errors::Error>>,
    ) -> String {
        spoken(bollard::exec::StartExecResults::Attached {
            output: Box::pin(tokio_stream::iter(chunks)),
            input: Box::pin(tokio::io::sink()),
        })
        .await
    }

    /// An attached exec is read through to the end, in the order it arrived.
    #[tokio::test]
    async fn every_chunk_an_exec_sent_is_gathered_in_order() {
        let said = attached(vec![
            Ok(bollard::container::LogOutput::StdOut {
                message: "one\n".into(),
            }),
            Ok(bollard::container::LogOutput::StdErr {
                message: "two\n".into(),
            }),
        ])
        .await;
        assert_eq!(said, "one\ntwo\n");
    }

    /// And a chunk that will not arrive ends it, rather than being passed over.
    ///
    /// The break stops the reading rather than being skipped: what arrived before it
    /// comes back, and what the stream would have said afterwards does not. Passing
    /// over a break and carrying on would splice two halves of an answer together
    /// with the gap invisible, which is the one shape worse than a short answer —
    /// the leak check compares what two of these said, and it can tell a short
    /// answer from an address.
    ///
    /// That this is not itself a refusal is [`gathered`]'s own reasoning: the exec's
    /// exit status is asked for immediately afterwards, and it is what says whether
    /// what arrived is an answer.
    #[tokio::test]
    async fn a_chunk_that_will_not_arrive_ends_the_gathering() {
        let said = attached(vec![
            Ok(bollard::container::LogOutput::StdOut {
                message: "some of it\n".into(),
            }),
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 500,
                message: "the stream stopped".to_owned(),
            }),
            Ok(bollard::container::LogOutput::StdOut {
                message: "and the rest\n".into(),
            }),
        ])
        .await;
        assert_eq!(said, "some of it\n", "the reading stops at the break");
    }
}
