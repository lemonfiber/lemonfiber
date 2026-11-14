//! A container engine that takes its time.

use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::docker::{
    Container, Engine, ExecOutput, Failure as EngineFailure, LogLine, LogQuery, Stats,
};
use tokio::sync::mpsc::Receiver;

use super::Reporting;

/// An engine that takes `listing` to list the containers and `inside` over every command
/// run inside one, answering as `engine` once it has.
pub struct Slowed {
    /// The engine that answers once the wait is over.
    pub engine: Reporting,
    /// How long listing the containers takes.
    pub listing: Duration,
    /// How long each command inside a container takes.
    pub inside: Duration,
}

#[async_trait]
impl Engine for Slowed {
    async fn list(&self, project: &str) -> Result<Vec<Container>, EngineFailure> {
        tokio::time::sleep(self.listing).await;
        self.engine.list(project).await
    }

    async fn exec(&self, container: &str, argv: &[String]) -> Result<ExecOutput, EngineFailure> {
        tokio::time::sleep(self.inside).await;
        self.engine.exec(container, argv).await
    }

    async fn stats(&self, project: &str) -> Result<Receiver<(String, Stats)>, EngineFailure> {
        self.engine.stats(project).await
    }

    async fn logs(
        &self,
        project: &str,
        services: &[String],
        query: LogQuery,
    ) -> Result<Receiver<LogLine>, EngineFailure> {
        self.engine.logs(project, services, query).await
    }
}
