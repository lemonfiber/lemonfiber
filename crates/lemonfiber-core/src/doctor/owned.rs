//! Whether every part that belongs to whoever claims it first is held by lemonfiber.
//!
//! The read-only half of the seeding step that claims them. Seeding claims each part
//! nobody holds; this asks each who holds it, changing nothing, so an operator running a
//! diagnosis is told of a part anybody can take, one somebody else holds, and one that
//! will not say, which is never taken as held.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::codes::config::PART_UNCLAIMED;
use crate::error::{Problem, Remedy};
use crate::ports::service::{Owner, Ownership};

/// The name this check and anything answering it share.
const CHECK: &str = "config.claimed";

/// The family each part's finding is named under.
const FAMILY: &str = "config.claimed.";

/// The heading an operator reads this under.
const TITLE: &str = "Who holds each service claimed first";

/// One part that belongs to whoever claims it first, as the check names it.
pub struct Part {
    /// Its id.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// The settings of the secrets lemonfiber holds for it.
    pub secrets: Vec<String>,
}

/// Reports who holds every part that belongs to whoever claims it first.
pub struct OwnedCheck {
    /// Every such part on this machine, each asked over its contract, or nothing where
    /// it cannot be asked.
    parts: Vec<(Part, Option<Arc<dyn Ownership>>)>,
}

impl OwnedCheck {
    /// A check over `parts`.
    #[must_use]
    pub fn new(parts: Vec<(Part, Option<Arc<dyn Ownership>>)>) -> Self {
        Self { parts }
    }
}

#[async_trait]
impl Check for OwnedCheck {
    fn category(&self) -> Category {
        Category::Config
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }
}

/// Who holds every part, where there is one to ask.
async fn ran(check: &OwnedCheck) -> Vec<Finding> {
    if check.parts.is_empty() {
        return vec![Finding::in_category(
            Category::Config,
            CHECK,
            TITLE,
            Verdict::Skipped {
                reason: "no service here belongs to whoever claims it first".to_owned(),
            },
        )];
    }
    let mut findings = Vec::with_capacity(check.parts.len());
    for (part, owner) in &check.parts {
        findings.push(
            Finding::in_category(
                Category::Config,
                &format!("{FAMILY}{}", part.id),
                &format!("Who holds {}", part.name),
                asked(part, owner.as_deref()).await,
            )
            .about(&part.id),
        );
    }
    findings
}

/// Who holds `part`, as a verdict.
async fn asked(part: &Part, owner: Option<&dyn Ownership>) -> Verdict {
    let name = part.name.as_str();
    let Some(owner) = owner else {
        return Verdict::Warn(unknown(name, "it cannot be asked over its contract"));
    };
    match owner.owner().await {
        Ok(Owner::Ours) => Verdict::Pass {
            note: Some(format!(
                "{name} is held with the credential lemonfiber minted for it"
            )),
        },
        Ok(Owner::Settling) => Verdict::Pass {
            note: Some(format!("{name} is taking up lemonfiber's claim")),
        },
        Ok(Owner::Open) => Verdict::Warn(Problem::new(
            PART_UNCLAIMED,
            format!("Nobody holds {name}"),
            crate::seed::run::owning::opened(name),
            Remedy::new(format!(
                "Run lemonfiber seed to claim {name}, or lemonfiber reset where lemonfiber \
                 claimed it before"
            )),
        )),
        Ok(Owner::Theirs) => Verdict::Warn(Problem::new(
            PART_UNCLAIMED,
            format!("Somebody else holds {name}"),
            crate::seed::run::owning::refusal(name, &part.secrets),
            Remedy::new(format!(
                "If that was not you, reset {name}'s configuration and run lemonfiber seed"
            )),
        )),
        Err(failure) => Verdict::Warn(unknown(name, &failure.to_string())),
    }
}

/// A part that would not say who holds it, which may be anybody.
fn unknown(name: &str, why: &str) -> Problem {
    Problem::new(
        PART_UNCLAIMED,
        format!("Anybody may hold {name}"),
        format!("{name} could not be asked who holds it: {why}"),
        Remedy::new(
            "Check the service is up and has finished starting, then run the diagnosis again",
        )
        .with_detail("lemonfiber status"),
    )
}

#[cfg(test)]
mod tests;
