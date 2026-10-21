//! Whether a part this build installs as a plugin still runs as the stack's own service,
//! each such part a move in place on offer.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::app::plugins::MoveOffered;
use crate::error::codes::plugin::MOVE_OFFERED;
use crate::error::{Problem, Remedy, State};

/// The name this check and anything answering it share.
const CHECK: &str = "services.move-offered";

/// The heading an operator reads this under.
const TITLE: &str = "Parts this release runs as plugins";

/// Where the moves on offer are read when the check runs.
#[async_trait]
pub trait Offers: Send + Sync {
    /// Each move in place on offer, and none where none is.
    ///
    /// # Errors
    ///
    /// `()` where what is installed cannot be read.
    async fn moves(&self) -> Result<Vec<MoveOffered>, ()>;
}

/// Names each part a move in place is on offer for.
pub struct MovesCheck {
    /// Where the moves on offer are read.
    offers: Arc<dyn Offers>,
}

impl MovesCheck {
    /// A check naming the moves `offers` has on offer when it runs.
    #[must_use]
    pub fn new(offers: Arc<dyn Offers>) -> Self {
        Self { offers }
    }
}

#[async_trait]
impl Check for MovesCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        vec![Finding::in_category(
            Category::Services,
            CHECK,
            TITLE,
            verdict(self.offers.moves().await),
        )]
    }
}

/// Pass where nothing is on offer, a warning naming each move otherwise, and unverified
/// where what is installed cannot be read.
fn verdict(moves: Result<Vec<MoveOffered>, ()>) -> Verdict {
    let Ok(moves) = moves else {
        return Verdict::Unverified {
            reason: "the record of what is installed could not be read, so whether a move in \
                     place is on offer is not known"
                .to_owned(),
            remedy: Remedy::new("Read what is installed, which says why the record cannot be read")
                .with_detail("lemonfiber plugin installed"),
        };
    };
    let Some(first) = moves.first() else {
        return Verdict::Pass { note: None };
    };
    let named: Vec<&str> = moves.iter().map(|one| one.plugin.as_str()).collect();
    Verdict::Warn(
        Problem::new(
            MOVE_OFFERED,
            format!("Moving {} onto its plugin is on offer", named.join(", ")),
            "This release runs each of these as a plugin, and this machine still runs it as the \
             stack's own service. It keeps running as it is until it is moved, and the move keeps \
             its container name, configuration, data and credentials.",
            Remedy::new("Read the move, and answer the name it prints")
                .with_detail(format!("lemonfiber plugin install {}", first.plugin)),
        )
        .in_state(State::Guided),
    )
}

#[cfg(test)]
mod tests;
