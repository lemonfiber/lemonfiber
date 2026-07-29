//! Running programs on this machine.

use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt as _, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{channel, Sender};

use crate::ports::process::{Failure, Output, Progress, Runner};

/// How many emitted lines may wait unread before the process is made to pause —
/// a display keeps up with a pull's output, so a small buffer is enough.
const BACKLOG: usize = 64;

/// Runs programs as child processes of this one.
#[derive(Debug, Default, Clone, Copy)]
pub struct Local;

#[async_trait]
impl Runner for Local {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
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

    async fn stream(
        &self,
        argv: &[String],
    ) -> Result<tokio::sync::mpsc::Receiver<Progress>, Failure> {
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
/// nothing.
async fn forward<R: AsyncRead + Unpin + Send + 'static>(reader: R, sender: Sender<Progress>) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let _ = sender.send(Progress::Line(line)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{Failure, Local, Progress, Runner};

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    /// Drain a stream into its lines and its final exit status.
    async fn drain(runner: &Local, parts: &[&str]) -> (Vec<String>, Option<i32>) {
        let Ok(mut stream) = runner.stream(&argv(parts)).await else {
            return (Vec::new(), None);
        };
        let mut lines = Vec::new();
        let mut status = None;
        while let Some(event) = stream.recv().await {
            match event {
                Progress::Line(line) => lines.push(line),
                Progress::Ended(code) => status = code,
            }
        }
        (lines, status)
    }

    #[tokio::test]
    async fn streams_a_programs_lines_then_its_exit() {
        // Both streams are read: a line to stdout and one to stderr, then a clean
        // exit. Order between the two is not asserted — only that both arrive.
        let (mut lines, status) = drain(
            &Local,
            &["sh", "-c", "printf 'out\\n'; printf 'err\\n' >&2"],
        )
        .await;
        lines.sort();
        assert_eq!(lines, vec!["err".to_owned(), "out".to_owned()]);
        assert_eq!(status, Some(0));
    }

    #[tokio::test]
    async fn draining_a_program_that_cannot_start_yields_nothing() {
        let (lines, status) = drain(&Local, &["lemonfiber-no-such-program"]).await;
        assert!(lines.is_empty());
        assert_eq!(status, None);
    }

    #[tokio::test]
    async fn a_non_zero_exit_arrives_on_the_stream_not_as_an_error() {
        let (_, status) = drain(&Local, &["sh", "-c", "exit 3"]).await;
        assert_eq!(
            status,
            Some(3),
            "the failed run's status still reaches the reader"
        );
    }

    #[tokio::test]
    async fn streaming_a_missing_program_is_distinguished_from_a_broken_one() {
        let result = Local.stream(&argv(&["lemonfiber-no-such-program"])).await;
        assert!(matches!(result, Err(Failure::NotFound { .. })));
    }

    #[tokio::test]
    async fn streaming_something_that_cannot_be_spawned_is_unusable_not_missing() {
        // A directory is present but not executable, so spawning it fails with a
        // reason other than "not found".
        let result = Local.stream(&argv(&["/"])).await;
        assert!(matches!(result, Err(Failure::Unusable { .. })));
    }

    #[tokio::test]
    async fn streaming_nothing_is_refused_rather_than_spawned() {
        assert!(matches!(
            Local.stream(&[]).await,
            Err(Failure::Unusable { .. })
        ));
    }

    #[tokio::test]
    async fn collects_what_a_program_wrote_and_how_it_exited() {
        let spoken = Local
            .run(&argv(&["echo", "hello"]))
            .await
            .map(|output| (output.succeeded(), output.stdout.trim().to_owned()))
            .ok();
        assert_eq!(spoken, Some((true, "hello".to_owned())));
    }

    #[tokio::test]
    async fn a_non_zero_exit_is_a_result_rather_than_an_error() {
        let outcome = Local
            .run(&argv(&["false"]))
            .await
            .map(|output| output.succeeded())
            .ok();
        assert_eq!(outcome, Some(false), "a failed program still ran");
    }

    #[tokio::test]
    async fn a_missing_program_is_distinguished_from_a_broken_one() {
        let result = Local
            .run(&argv(&["lemonfiber-no-such-program-exists"]))
            .await;
        assert!(matches!(result, Err(Failure::NotFound { .. })));
    }

    #[tokio::test]
    async fn an_empty_argument_vector_is_refused_rather_than_spawned() {
        assert!(matches!(
            Local.run(&[]).await,
            Err(Failure::Unusable { .. })
        ));
    }
}
