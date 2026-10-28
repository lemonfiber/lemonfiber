//! Whether a service is held back until the operator gives what it needs to start.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::codes::setting::HELD_BACK;
use crate::error::{Problem, Remedy, State};
use crate::model::Waiting;

/// The name this check and anything answering it share.
const CHECK: &str = "services.waiting";

/// The heading an operator reads this under.
const TITLE: &str = "Services waiting for what only you can give";

/// Where the services held back are read when the check runs.
#[async_trait]
pub trait Holds: Send + Sync {
    /// Each service held back, with what it waits for, and none where none is.
    ///
    /// # Errors
    ///
    /// `()` where what is installed cannot be read.
    async fn waiting(&self) -> Result<Vec<Waiting>, ()>;
}

/// Names each service held back and what it waits for.
pub struct WaitingCheck {
    /// Where the services held back are read.
    holding: Arc<dyn Holds>,
}

impl WaitingCheck {
    /// A check naming the services `holding` holds back when it runs.
    #[must_use]
    pub fn new(holding: Arc<dyn Holds>) -> Self {
        Self { holding }
    }
}

#[async_trait]
impl Check for WaitingCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        vec![Finding::in_category(
            Category::Services,
            CHECK,
            TITLE,
            verdict(self.holding.waiting().await),
        )]
    }
}

/// Pass where nothing waits, a warning naming each service and what it waits for
/// otherwise, and unverified where what is installed cannot be read.
fn verdict(waiting: Result<Vec<Waiting>, ()>) -> Verdict {
    let Ok(waiting) = waiting else {
        return Verdict::Unverified {
            reason: "the record of what is installed could not be read, so whether a service \
                     waits for a setting or a secret is not known"
                .to_owned(),
            remedy: Remedy::new("Read what is installed, which says why the record cannot be read")
                .with_detail("lemonfiber plugin installed"),
        };
    };
    if waiting.is_empty() {
        return Verdict::Pass { note: None };
    }
    let services: Vec<&str> = waiting.iter().map(|one| one.service.as_str()).collect();
    let said: Vec<String> = waiting.iter().map(Waiting::said).collect();
    let many = services.len() > 1;
    Verdict::Warn(
        Problem::new(
            HELD_BACK,
            format!(
                "{} {} held back until you give what {} to start",
                services.join(", "),
                if many { "are" } else { "is" },
                if many { "they need" } else { "it needs" }
            ),
            format!(
                "{}. The rest of the stack runs; each starts once the last thing it waits for \
                 is given.",
                said.join(". ")
            ),
            Remedy::new("Give each setting a service waits for")
                .with_detail("lemonfiber settings set <setting>=<value>"),
        )
        .in_state(State::Guided),
    )
}

#[cfg(test)]
mod tests;
