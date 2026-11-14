//! How `Reporting` answers each question the engine port asks.

use lemonfiber_ports::docker::{
    Container, ExecOutput, Failure as EngineFailure, Health, LogLine, LogQuery, Stats,
};
use tokio::sync::mpsc::Receiver;

use super::{Question, Reporting};

/// How `Reporting` answers `list`.
pub(super) async fn list(fake: &Reporting, project: &str) -> Result<Vec<Container>, EngineFailure> {
    fake.hang().await;
    let Some(engine) = &fake.over else {
        return listed(fake, project);
    };
    let mut running = engine.list(project).await.unwrap_or_default();
    running.extend(listed(fake, project)?);
    Ok(running)
}

/// How `Reporting` answers `exec`.
pub(super) async fn exec(
    fake: &Reporting,
    container: &str,
    argv: &[String],
) -> Result<ExecOutput, EngineFailure> {
    fake.hang().await;
    if let Some(engine) = &fake.over {
        return engine.exec(container, argv).await;
    }
    fake.answers(Question::Exec)?;
    match &fake.exec {
        Some(output) => Ok(output.clone()),
        None => executed(fake, container, argv),
    }
}

/// How `Reporting` answers `stats`.
pub(super) async fn stats(
    fake: &Reporting,
    project: &str,
) -> Result<Receiver<(String, Stats)>, EngineFailure> {
    fake.hang().await;
    if let Some(engine) = &fake.over {
        return engine.stats(project).await;
    }
    fake.answers(Question::Stats)?;
    let (_sender, receiver) = tokio::sync::mpsc::channel(1);
    Ok(receiver)
}

/// How `Reporting` answers `logs`.
pub(super) async fn reporting_logs(
    fake: &Reporting,
    project: &str,
    services: &[String],
    query: LogQuery,
) -> Result<Receiver<LogLine>, EngineFailure> {
    fake.hang().await;
    if let Some(engine) = &fake.over {
        return engine.logs(project, services, query).await;
    }
    fake.answers(Question::Logs)?;
    logs(fake, services).await
}

impl Reporting {
    /// Never coming back, where this engine hangs.
    async fn hang(&self) {
        if self.hangs {
            std::future::pending::<()>().await;
        }
    }

    /// Refused as an engine not answering `question`, where it refuses that.
    fn answers(&self, question: Question) -> Result<(), EngineFailure> {
        if self.refusing.contains(&question) {
            return Err(EngineFailure::Unreachable {
                reason: "nothing is running here".to_owned(),
            });
        }
        Ok(())
    }
}

/// The containers of one Compose project, as the engine would answer, with health settled
/// once it has been asked enough.
///
/// Narrowed to `project` because the real adapter narrows: it asks the daemon for
/// containers carrying that project's label, so a fake that answered with every
/// container it held would let a caller pass a proof it never actually made. The
/// containers a fake is built with belong to lemonfiber's own project unless a test
/// said otherwise, so this filter is invisible to every test that never mentioned
/// one.
fn listed(reporting: &Reporting, project: &str) -> Result<Vec<Container>, EngineFailure> {
    if !reporting.reachable {
        return Err(EngineFailure::Unreachable {
            reason: "no daemon here".to_owned(),
        });
    }

    if reporting.answers.is_some_and(|answers| {
        reporting
            .answered
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            >= answers
    }) {
        return Err(EngineFailure::Unreachable {
            reason: "the daemon has gone".to_owned(),
        });
    }

    // Refused before anything is counted against the settling budget, so an engine
    // that was still starting arrives with the same patience for its containers as
    // one that was there from the outset.
    if reporting
        .refused
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        < reporting.refusals
    {
        return Err(EngineFailure::Unreachable {
            reason: "the daemon is still starting".to_owned(),
        });
    }

    let asked = reporting
        .asked
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let settled = reporting.settles_after.is_some_and(|after| asked >= after);

    Ok(reporting
        .containers
        .iter()
        .filter(|container| container.project == project)
        .map(|container| Container {
            health: if settled {
                Health::Healthy
            } else {
                container.health
            },
            ..container.clone()
        })
        .collect())
}

/// Which scripted answer a command inside a container gets.
fn executed(
    reporting: &Reporting,
    container: &str,
    argv: &[String],
) -> Result<ExecOutput, EngineFailure> {
    let Some(tunnel) = &reporting.tunnel else {
        return Err(EngineFailure::NoSuchContainer {
            name: container.to_owned(),
        });
    };
    if argv.last().is_some_and(|arg| arg.ends_with("/country-iso")) {
        return Ok(ExecOutput {
            status: Some(0),
            stdout: tunnel.country.unwrap_or_default().to_owned(),
        });
    }
    let asked_second = argv
        .last()
        .is_some_and(|arg| arg.contains("second.example"));
    let ip = if container.contains(tunnel.gateway) {
        if asked_second {
            tunnel.second_opinion.or(tunnel.gateway_ip)
        } else {
            tunnel.gateway_ip
        }
    } else {
        tunnel.client_ip
    };
    Ok(super::scripted(ip))
}

async fn logs(
    reporting: &Reporting,
    services: &[String],
) -> Result<Receiver<LogLine>, EngineFailure> {
    // Opening a stream means finding the containers first, so an engine
    // that cannot be reached refuses this as surely as it refuses a
    // listing. A fake that answered anyway would be a fake that made
    // the health gate's own failure path untestable.
    if !reporting.reachable {
        return Err(EngineFailure::Unreachable {
            reason: "no daemon here".to_owned(),
        });
    }

    let wanted: Vec<LogLine> = reporting
        .said
        .iter()
        .filter(|line| services.is_empty() || services.contains(&line.service))
        .cloned()
        .collect();

    let (sender, receiver) = tokio::sync::mpsc::channel(wanted.len().max(1));
    for line in wanted {
        let _ = sender.send(line).await;
    }
    Ok(receiver)
}
