//! The bundled egress guard, answering `network.egress-guard` from its own container.

use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::docker::{Engine, Health, Lifecycle};
use crate::ports::service::{Failure, Tunnelling};

use super::port_forward::Grant;
use super::probe::{find, read_grant};

/// The bundled Gluetun, asked through the container it runs as.
pub(crate) struct Gluetun {
    engine: Arc<dyn Engine>,
    project: String,
    service: String,
    forwarding: bool,
}

impl Gluetun {
    /// The guard running as `service` in `project`, forwarding a port only where the
    /// operator asked it to.
    pub(crate) fn new(
        engine: Arc<dyn Engine>,
        project: String,
        service: String,
        forwarding: bool,
    ) -> Self {
        Self {
            engine,
            project,
            service,
            forwarding,
        }
    }

    fn unavailable(&self) -> Failure {
        Failure::Unavailable {
            service: self.service.clone(),
        }
    }
}

#[async_trait]
impl Tunnelling for Gluetun {
    async fn up(&self) -> Result<bool, Failure> {
        up(self).await
    }

    async fn forwarded(&self) -> Result<Option<u16>, Failure> {
        forwarded(self).await
    }
}

/// Whether the guard's container runs and its own probe is not failing.
async fn up(guard: &Gluetun) -> Result<bool, Failure> {
    let containers = guard
        .engine
        .list(&guard.project)
        .await
        .map_err(|_| guard.unavailable())?;
    Ok(find(&containers, &guard.service).is_some_and(|container| {
        container.lifecycle == Lifecycle::Running && container.health != Health::Unhealthy
    }))
}

/// The port in the guard's own status file, or none where it was not asked to forward.
async fn forwarded(guard: &Gluetun) -> Result<Option<u16>, Failure> {
    if !guard.forwarding {
        return Ok(None);
    }
    let containers = guard
        .engine
        .list(&guard.project)
        .await
        .map_err(|_| guard.unavailable())?;
    match read_grant(guard.engine.as_ref(), find(&containers, &guard.service)).await {
        Grant::Port(port) => Ok(Some(port)),
        Grant::Absent => Ok(None),
        Grant::Unreadable => Err(guard.unavailable()),
    }
}

#[cfg(test)]
mod tests;
