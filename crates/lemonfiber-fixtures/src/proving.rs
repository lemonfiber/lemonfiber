//! An engine that runs an image's own program as a test scripts it.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::program::{Ended, Program, Ran, Run, Unran};

/// Answers every run the same way, or by its first argument where it is told to, and
/// keeps every run it was asked for.
pub struct Proving {
    answer: Result<Ran, Unran>,
    commanded: Vec<(String, Result<Ran, Unran>)>,
    asked: Mutex<Vec<Run>>,
}

impl Proving {
    /// A run that exited with `exit`, having written `stdout`, as an engine answers it.
    ///
    /// # Errors
    ///
    /// Never: the run it answers was made and exited.
    pub fn exited(exit: i64, stdout: &str) -> Result<Ran, Unran> {
        Ok(Ran {
            ended: Ended::Exited(exit),
            stdout: stdout.as_bytes().to_vec(),
        })
    }

    /// Every run exits with `exit`, having written `stdout`.
    #[must_use]
    pub fn exiting(exit: i64, stdout: &str) -> Arc<Self> {
        Self::answering(Self::exited(exit, stdout))
    }

    /// Every run is still going at its limit, having written `stdout`.
    #[must_use]
    pub fn overrunning(stdout: &str) -> Arc<Self> {
        Self::answering(Ok(Ran {
            ended: Ended::Overran,
            stdout: stdout.as_bytes().to_vec(),
        }))
    }

    /// No engine answers.
    #[must_use]
    pub fn unreachable() -> Arc<Self> {
        Self::answering(Err(Unran::Unreachable {
            reason: "no engine in a test".to_owned(),
        }))
    }

    /// Every run answered with `answer`.
    #[must_use]
    pub fn answering(answer: Result<Ran, Unran>) -> Arc<Self> {
        Self::commanded(Vec::new(), answer)
    }

    /// Each run whose first argument one of `commanded` names answered with its answer,
    /// and every other with `rest`.
    #[must_use]
    pub fn commanded(
        commanded: Vec<(&str, Result<Ran, Unran>)>,
        rest: Result<Ran, Unran>,
    ) -> Arc<Self> {
        Arc::new(Self {
            answer: rest,
            commanded: commanded
                .into_iter()
                .map(|(command, answer)| (command.to_owned(), answer))
                .collect(),
            asked: Mutex::new(Vec::new()),
        })
    }

    /// The answer `run`'s first argument is given, with `run` kept among those asked for.
    fn answering_to(&self, run: &Run) -> Result<Ran, Unran> {
        if let Ok(mut held) = self.asked.lock() {
            held.push(run.clone());
        }
        self.commanded
            .iter()
            .find(|(command, _)| run.args.first() == Some(command))
            .map_or(&self.answer, |(_, answer)| answer)
            .clone()
    }

    /// Every run asked for, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<Run> {
        self.asked
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Program for Proving {
    async fn run(&self, run: &Run) -> Result<Ran, Unran> {
        self.answering_to(run)
    }
}
