//! Running programs on this machine.

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc::{channel, Sender};

use lemonfiber_ports::process::{Failure, Output, Progress, Runner};

/// How many emitted lines may wait unread before the process is made to pause —
/// a display keeps up with a pull's output, so a small buffer is enough.
const BACKLOG: usize = 64;

/// How long a program run to completion may take before it is stopped.
///
/// Two hours, sized against the longest thing run this way: an update pulls each
/// service's new image through one of these, and an image of a few hundred
/// megabytes over a slow line takes most of an hour. Past this a program is hung
/// rather than slow, and stopping it gives the run back to whoever is waiting on it.
const RUN_WITHIN: Duration = Duration::from_secs(2 * 60 * 60);

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
///
/// The child is killed if this is dropped before it ends — work ended part-way
/// takes the program it was running with it rather than leaving it to finish on
/// its own — and when it outlasts [`RUN_WITHIN`]. Either way what it had written is
/// kept, and a program stopped this way has no exit status, which is what every
/// caller already reads as a run that did not succeed.
async fn ran(argv: &[String]) -> Result<Output, Failure> {
    let Some((program, arguments)) = argv.split_first() else {
        return Err(Failure::Unusable {
            program: String::new(),
            reason: "no program was given".to_owned(),
        });
    };

    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| started(program, &err))?;

    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let status = waited(&mut child, &mut stdout, &mut stderr).await;
    Ok(Output {
        status,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

/// Read a child's output out and wait for it to end, for [`RUN_WITHIN`] at most.
///
/// What it writes is read into buffers this borrows rather than owns, so what had
/// arrived when the time ran out is still there once the wait is abandoned.
async fn waited(child: &mut Child, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> Option<i32> {
    let (out, err) = (child.stdout.take(), child.stderr.take());
    let ending = async {
        let ((), (), exit) = tokio::join!(drained(out, stdout), drained(err, stderr), child.wait());
        exit.ok().and_then(|exit| exit.code())
    };
    match tokio::time::timeout(RUN_WITHIN, ending).await {
        Ok(status) => status,
        Err(_overran) => {
            let _killed = child.kill().await;
            None
        }
    }
}

/// Everything a reader produces, appended a read at a time so a read abandoned
/// part-way leaves what came before it in place.
async fn drained<R: AsyncRead + Unpin>(reader: Option<R>, into: &mut Vec<u8>) {
    if let Some(mut reader) = reader {
        while reader.read_buf(into).await.is_ok_and(|read| read > 0) {}
    }
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
/// has walked away is let go rather than acted on: the task that owns the child
/// kills it once the receiver is gone, and the reading ends when its output does.
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
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| started(program, &err))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (sender, receiver) = channel(BACKLOG);
    // One task owns the child; a reader task per stream sends lines as they
    // land, so stderr (where compose writes its progress) is not held back
    // until stdout closes. The exit follows once both streams are spent.
    //
    // A receiver that goes away first is work that was ended part-way, and the
    // child goes with it: dropped here, it is killed rather than left to finish a
    // start or a pull nobody is waiting on any more.
    tokio::spawn(async move {
        let mut readers = Vec::new();
        if let Some(out) = stdout {
            readers.push(tokio::spawn(forward(out, sender.clone())));
        }
        if let Some(err) = stderr {
            readers.push(tokio::spawn(forward(err, sender.clone())));
        }
        let ending = async {
            for reader in readers {
                let _ = reader.await;
            }
            child.wait().await.ok().and_then(|exit| exit.code())
        };
        tokio::select! {
            status = ending => {
                let _ = sender.send(Progress::Ended(status)).await;
            }
            () = sender.closed() => {}
        }
    });
    Ok(receiver)
}

#[cfg(test)]
mod tests;
