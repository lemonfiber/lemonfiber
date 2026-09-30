//! Which containers lemonfiber stopped itself, so a stop it made reads as one.
//!
//! The engine keeps how a container exited and nothing about who asked it to. Plenty of
//! services exit non-zero when told to stop — a tunnel that treats the signal as an
//! error, anything killed when its grace period runs out — so a reading that went by the
//! exit code alone would show the operator's own stop back to them as a fault. lemonfiber
//! knows which containers it stopped, because it stopped them, and writes that down here.
//!
//! **Written after a stop and let go of before a start.** A container started again is
//! the same container, and one that falls over after that fell over by itself. So every
//! start lets go of the services it addresses before anything is run: a start that fails
//! part way must read as the failure it is, not as the stop that came before it. Letting
//! go of more than was stopped is always safe — it only returns a container to being read
//! by its own exit code.
//!
//! Kept beside the configuration, best effort, like the other small records there. One
//! that could not be written leaves a stop reading as a failure, which is a worse picture
//! rather than a wrong claim.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::docker::Halted;
use crate::ports::docker::Lifecycle;
use crate::stack::compose::Action;

use super::super::Ctx;

/// The record's file name, named once so the reader and the writers agree.
const RECORD: &str = "halted.json";

/// The containers lemonfiber stopped, by the service each belongs to.
///
/// By service as well as by container, because a start names services: letting go of
/// what a start addresses is a question about services, and reading a container is a
/// question about containers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    /// Each service's stopped containers, by the engine's id for each.
    services: BTreeMap<String, Vec<String>>,
}

/// What lemonfiber has stopped, for a reading of the engine to take into account.
#[must_use]
pub(crate) fn load(ctx: &Ctx) -> Halted {
    let record: Record = crate::app::record::beside(ctx, RECORD);
    Halted::new(record.services.into_values().flatten())
}

/// Let go of the services an action is about to start, before it runs.
///
/// Every action that brings a container back or takes it away lets go: a start, a
/// restart, and a removal alike. Only a stop is written down, and the actions that run
/// nothing leave the record alone.
pub(crate) fn before(ctx: &Ctx, action: &Action, addressed: &[String]) {
    if matches!(action, Action::Stop(_) | Action::Pull | Action::Config) {
        return;
    }
    let mut record: Record = crate::app::record::beside(ctx, RECORD);
    let before = record.services.len();
    record
        .services
        .retain(|service, _| !addressed.contains(service));
    if record.services.len() != before {
        crate::app::record::keep_beside(ctx, RECORD, &record);
    }
}

/// Write down the containers a stop just stopped.
///
/// Read off the engine after the stop rather than assumed from the command, so what is
/// written is the containers that are actually down, by the ids the engine will report
/// them under next time. An engine that will not answer leaves the record as it was.
pub(crate) async fn after(ctx: &Ctx, action: &Action, addressed: &[String]) {
    if !matches!(action, Action::Stop(_)) {
        return;
    }
    let Ok(containers) = ctx.seams.engine.list(&ctx.settings.project).await else {
        return;
    };
    let mut record: Record = crate::app::record::beside(ctx, RECORD);
    for service in addressed {
        let stopped: Vec<String> = containers
            .iter()
            .filter(|container| &container.service == service)
            .filter(|container| {
                matches!(
                    container.lifecycle,
                    Lifecycle::Exited | Lifecycle::Dead | Lifecycle::Removing
                )
            })
            .map(|container| container.id.clone())
            .collect();
        if !stopped.is_empty() {
            record.services.insert(service.clone(), stopped);
        }
    }
    crate::app::record::keep_beside(ctx, RECORD, &record);
}

#[cfg(test)]
mod tests;
