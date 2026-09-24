//! How far the stack got towards being askable.
//!
//! A ladder rather than a handful of booleans, so the states are exclusive by
//! construction: a machine is at exactly one rung, and there is no way to describe
//! one that is both unconfigured and running.

use serde::{Deserialize, Serialize};

use crate::docker::{condition, Condition, Service, State};

/// How far the stack got towards being askable, which is not the same question as
/// whether anything is wrong with it.
///
/// One ladder for the whole crate. The dashboard's standing and the health summary
/// both read from it, because a surface that decided the engine was unreachable and
/// a summary that decided the stack was fine would be describing the same machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reach {
    /// Nothing has been set up yet.
    Unconfigured,
    /// Set up, and deliberately not running.
    Stopped,
    /// Coming up. Not yet a verdict on anything.
    Starting,
    /// Up, and answering.
    Running,
    /// Should be up; could not be asked.
    Unreachable,
}

impl Reach {
    /// Which rung a machine is on, from whether it is configured and what the
    /// engine said about its services.
    ///
    /// `None` for the services is an engine that could not be asked — distinct
    /// from an empty list, which is an engine that answered and reported nothing.
    /// Read here rather than at each surface, so a dashboard and a status command
    /// cannot place the same machine on two different rungs.
    #[must_use]
    pub fn of(configured: bool, services: Option<&[Service]>) -> Self {
        if !configured {
            return Self::Unconfigured;
        }
        let Some(services) = services else {
            return Self::Unreachable;
        };
        if condition(services) == Condition::Inactive {
            return Self::Stopped;
        }
        // Still inside its probe's start period, with nothing yet failed: coming up,
        // which is not a verdict on anything.
        if services
            .iter()
            .any(|service| service.state == State::Starting)
            && !services
                .iter()
                .any(|service| service.state.wants_attention())
        {
            return Self::Starting;
        }
        Self::Running
    }
}

#[cfg(test)]
mod tests;
