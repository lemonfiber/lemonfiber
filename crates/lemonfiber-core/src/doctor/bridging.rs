//! Whether `OrbStack`, where it is the stack's engine, mirrors every container network into
//! macOS: the setting that makes each container start slow on a stack holding a network
//! for every link between two of its services.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::codes::env::NETWORKS_MIRRORED;
use crate::error::{Problem, Remedy, State};
use crate::platform::Environment;
use crate::ports::docker::Target;
use crate::ports::Runner;

/// The check's stable identifier.
const CHECK: &str = "environment.network-bridge";

/// What the check is called in front of the operator.
const TITLE: &str = "OrbStack network bridge";

/// The setting `OrbStack`'s configuration names the bridge by.
const SETTING: &str = "network_bridge";

/// What turns the bridge off.
const TURN_OFF: &str = "Run orb config set network_bridge false, then restart OrbStack";

/// Whether the stack's engine mirrors its container networks into macOS.
pub struct BridgingCheck {
    runner: Arc<dyn Runner>,
    target: Target,
}

impl BridgingCheck {
    /// A check over the given way of running programs, about the engine this run operates.
    #[must_use]
    pub fn new(runner: Arc<dyn Runner>, target: Target) -> Self {
        Self { runner, target }
    }
}

#[async_trait]
impl Check for BridgingCheck {
    fn category(&self) -> Category {
        Category::Environment
    }

    async fn run(&self) -> Vec<Finding> {
        vec![Finding::in_category(
            Category::Environment,
            CHECK,
            TITLE,
            ran(self).await,
        )]
    }
}

/// The command that shows `OrbStack`'s configuration.
fn shown() -> Vec<String> {
    ["orb", "config", "show"].map(str::to_owned).to_vec()
}

/// What the bridge setting is, where the engine is `OrbStack`'s.
async fn ran(check: &BridgingCheck) -> Verdict {
    if !Environment::engine_is_orbstack(&check.target) {
        return Verdict::Skipped {
            reason: "the stack's engine is not OrbStack".to_owned(),
        };
    }
    let said = match check.runner.run(&shown()).await {
        Ok(output) if output.succeeded() => bridged(&output.stdout),
        _ => None,
    };
    verdict(said)
}

/// Whether `shown` says the bridge is on, or nothing where it does not say.
fn bridged(shown: &str) -> Option<bool> {
    shown.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.trim() == SETTING)
            .then(|| match value.trim() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            })
            .flatten()
    })
}

/// The verdict for what `OrbStack` said about its bridge.
fn verdict(bridged: Option<bool>) -> Verdict {
    match bridged {
        Some(false) => Verdict::Pass {
            note: Some("OrbStack does not mirror the stack's networks into the Mac".to_owned()),
        },
        Some(true) => Verdict::Warn(
            Problem::new(
                NETWORKS_MIRRORED,
                "OrbStack mirrors every container network into the Mac",
                "The stack gives every link between two services a network of its own, and \
                 with each of them mirrored a container start can take tens of seconds. \
                 lemonfiber reaches every service through a published port, so nothing of the \
                 stack needs the bridge.",
                Remedy::new(TURN_OFF),
            )
            .in_state(State::Guided),
        ),
        None => Verdict::Unverified {
            reason: format!(
                "OrbStack's configuration could not be read, so whether {SETTING} is on cannot \
                 be told"
            ),
            remedy: Remedy::new(format!(
                "Run orb config show and look for {SETTING}. Where it says true, turn it off: \
                 {TURN_OFF}"
            )),
        },
    }
}

#[cfg(test)]
mod tests;
