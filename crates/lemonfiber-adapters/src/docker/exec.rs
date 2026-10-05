//! Running one command inside a container, and keeping what it said within bounds.
//!
//! Every command this asks a container to run is a short question: an address
//! fetched, a status file read, a route listed. One that does not end, or that
//! writes without stopping, is a container misbehaving rather than an answer
//! taking its time, and a caller waiting on it would wait for as long as the
//! container chose while the memory holding what it wrote grew with it.

use std::time::Duration;

use tokio_stream::StreamExt as _;

use lemonfiber_ports::docker::{ExecOutput, Failure};

use super::{unreachable, Daemon};

/// How long a command's output is read before it is taken as everything it said.
///
/// Thirty seconds, against questions that answer in well under one. A command
/// still writing at the end of it is reported with whatever it wrote and no exit
/// status, which is what every caller already reads as a command that did not
/// succeed.
pub(super) const EXEC_WITHIN: Duration = Duration::from_secs(30);

/// The most of a command's output that is kept.
///
/// Sixty-four kilobytes, several hundred times what an address or a status file
/// holds. What arrives past it is not read, so a command that writes without
/// stopping costs this much memory and no more.
pub(super) const EXEC_SAID_AT_MOST: usize = 64 * 1024;

/// Run one command inside a container and collect what it said.
pub(super) async fn executed(
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
pub(super) fn refused_exec(error: bollard::errors::Error, container: &str) -> Failure {
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
///
/// Written as a value that starts empty and is filled where there is something to
/// read, rather than as two arms. The other arm is the detached one, and this adapter
/// never asks to detach — so as an arm of its own it is a line no run can enter,
/// which is a line the coverage gate counts against every honest line beside it. As
/// an absence it says the same thing: an exec nobody attached to wrote nothing here.
///
/// Read for [`EXEC_WITHIN`] at most. The reading borrows what it fills rather than
/// returning it, so what had arrived when the time ran out is still here to hand
/// back.
pub(super) async fn spoken(started: bollard::exec::StartExecResults) -> String {
    let mut said = String::new();
    if let bollard::exec::StartExecResults::Attached { output, .. } = started {
        let _within = tokio::time::timeout(EXEC_WITHIN, gathered(output, &mut said)).await;
    }
    said
}

/// Everything a stream of exec output said, joined in the order it arrived, up to
/// [`EXEC_SAID_AT_MOST`].
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
async fn gathered<S>(mut output: S, said: &mut String)
where
    S: tokio_stream::Stream<Item = Result<bollard::container::LogOutput, bollard::errors::Error>>
        + Unpin,
{
    while let Some(Ok(chunk)) = output.next().await {
        let chunk = chunk.to_string();
        let room = EXEC_SAID_AT_MOST.saturating_sub(said.len());
        if chunk.len() > room {
            said.push_str(within(&chunk, room));
            return;
        }
        said.push_str(&chunk);
    }
}

/// The longest start of `text` that is at most `bytes` long and ends on a character.
fn within(text: &str, bytes: usize) -> &str {
    let mut end = bytes.min(text.len());
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    text.get(..end).unwrap_or_default()
}
