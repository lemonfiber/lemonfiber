//! Running programs on this machine.

use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt as _, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{channel, Sender};

use lemonfiber_ports::process::{Failure, Output, Progress, Runner};

/// How many emitted lines may wait unread before the process is made to pause —
/// a display keeps up with a pull's output, so a small buffer is enough.
const BACKLOG: usize = 64;

/// Runs programs as child processes of this one.
#[derive(Debug, Default, Clone, Copy)]
pub struct Local;

#[async_trait]
impl Runner for Local {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        ran(argv).await
    }

    async fn stream(
        &self,
        argv: &[String],
    ) -> Result<tokio::sync::mpsc::Receiver<Progress>, Failure> {
        stream(argv)
    }
}

/// Spawn the program and wait for it, or say why it could not be spawned.
async fn ran(argv: &[String]) -> Result<Output, Failure> {
    let Some((program, arguments)) = argv.split_first() else {
        return Err(Failure::Unusable {
            program: String::new(),
            reason: "no program was given".to_owned(),
        });
    };

    let output = Command::new(program)
        .args(arguments)
        .output()
        .await
        .map_err(|err| started(program, &err))?;

    Ok(Output {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Turn an I/O error from spawning a program into the failure that names its
/// cause — a program that is not installed apart from one that will not start,
/// because the remedies differ.
fn started(program: &str, err: &std::io::Error) -> Failure {
    if err.kind() == std::io::ErrorKind::NotFound {
        Failure::NotFound {
            program: program.to_owned(),
        }
    } else {
        Failure::Unusable {
            program: program.to_owned(),
            reason: err.to_string(),
        }
    }
}

/// Send each line a reader produces until it is spent. A send to a receiver that
/// has walked away is let go rather than acted on: the process is finite and
/// already running, so reading it out costs nothing and stopping early buys
/// nothing. This is why `stream` is for finite commands only — a pull, not a
/// `--follow`: a dropped receiver does not stop the child, so an endless one would
/// leave this task reading forever.
async fn forward<R: AsyncRead + Unpin + Send + 'static>(reader: R, sender: Sender<Progress>) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let _ = sender.send(Progress::Line(line)).await;
    }
}

/// Nothing of the runner's is read: a stream is the child it spawns and the two
/// readers draining it, and the runner holds nothing that changes either.
fn stream(argv: &[String]) -> Result<tokio::sync::mpsc::Receiver<Progress>, Failure> {
    let Some((program, arguments)) = argv.split_first() else {
        return Err(Failure::Unusable {
            program: String::new(),
            reason: "no program was given".to_owned(),
        });
    };

    let mut child = Command::new(program)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| started(program, &err))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (sender, receiver) = channel(BACKLOG);
    // One task owns the child; a reader task per stream sends lines as they
    // land, so stderr (where compose writes its progress) is not held back
    // until stdout closes. The exit follows once both streams are spent.
    tokio::spawn(async move {
        let mut readers = Vec::new();
        if let Some(out) = stdout {
            readers.push(tokio::spawn(forward(out, sender.clone())));
        }
        if let Some(err) = stderr {
            readers.push(tokio::spawn(forward(err, sender.clone())));
        }
        for reader in readers {
            let _ = reader.await;
        }
        let status = child.wait().await.ok().and_then(|exit| exit.code());
        let _ = sender.send(Progress::Ended(status)).await;
    });
    Ok(receiver)
}

#[cfg(test)]
mod tests;
