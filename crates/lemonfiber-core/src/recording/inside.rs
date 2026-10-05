//! What a container was asked to fetch, written down beside what this machine sent.
//!
//! The address the tunnel leaves by is learned by asking a container to fetch it,
//! from inside its own network, and that fetch is the request that most often
//! leaves the house. It never passes through this program's HTTP transport, so the
//! record kept there would never see it. This sits around the container engine
//! instead and writes down every command that names an address to fetch.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

use super::{leaves_this_machine, stamped, Ledger};
use crate::error::withheld::{withheld, without_credentials};
use crate::ports::docker::{Container, Engine, ExecOutput, Failure, LogLine, LogQuery, Stats};
use crate::ports::time::Clock;

/// A container engine whose fetches from inside a container are written down.
pub struct Inside {
    /// The engine everything is asked of.
    inner: Arc<dyn Engine>,
    /// Where the record is kept.
    ledger: Arc<Ledger>,
    /// What the time is, asked through the port so a test can say.
    clock: Arc<dyn Clock>,
}

impl Inside {
    /// Wrap an engine so what its containers are asked to fetch is written down.
    #[must_use]
    pub fn around(inner: Arc<dyn Engine>, ledger: Arc<Ledger>, clock: Arc<dyn Clock>) -> Self {
        Self {
            inner,
            ledger,
            clock,
        }
    }
}

/// The addresses a command names, which is what a fetch run inside a container
/// sends a request to.
fn fetched(argv: &[String]) -> impl Iterator<Item = &String> {
    argv.iter()
        .filter(|word| word.starts_with("http://") || word.starts_with("https://"))
        .filter(|address| leaves_this_machine(address))
}

/// One line of the record for a fetch made from inside a container.
///
/// The same four parts as a line for a request this machine sent — when, what kind,
/// where with its credentials withheld, what came back — and then where it was sent
/// from, because a reader checking the record against what they allowed needs to
/// know this one did not leave by this machine's own address.
fn line(at: u64, address: &str, output: Option<&ExecOutput>) -> String {
    let outcome = match output.and_then(|output| output.status) {
        Some(0) => "answered",
        _ => "nothing answered",
    };
    format!(
        "{at} Get {} {outcome} from inside a container",
        withheld(&without_credentials(address))
    )
}

#[async_trait]
impl Engine for Inside {
    async fn list(&self, project: &str) -> Result<Vec<Container>, Failure> {
        self.inner.list(project).await
    }

    async fn exec(&self, container: &str, argv: &[String]) -> Result<ExecOutput, Failure> {
        executed(self, container, argv).await
    }

    async fn stats(&self, project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
        self.inner.stats(project).await
    }

    async fn logs(
        &self,
        project: &str,
        services: &[String],
        query: LogQuery,
    ) -> Result<Receiver<LogLine>, Failure> {
        self.inner.logs(project, services, query).await
    }
}

/// Run the command, and write down each address it was asked to fetch.
async fn executed(
    inside: &Inside,
    container: &str,
    argv: &[String],
) -> Result<ExecOutput, Failure> {
    let ran = inside.inner.exec(container, argv).await;
    let at = stamped(inside.clock.as_ref());
    for address in fetched(argv) {
        inside
            .ledger
            .note(line(at, address, ran.as_ref().ok()))
            .await;
    }
    ran
}

#[cfg(test)]
mod tests;
