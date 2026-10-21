//! Whether a part this build installs as a plugin still runs as the stack's own service,
//! each such part a move in place on offer (ARCH-R299).

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::app::plugins::MoveOffered;
use crate::error::codes::plugin::MOVE_OFFERED;
use crate::error::{Problem, Remedy, State};

/// The name this check and anything answering it share.
const CHECK: &str = "services.move-offered";

/// The heading an operator reads this under.
const TITLE: &str = "Parts this release runs as plugins";

/// Names each part a move in place is on offer for.
pub struct MovesCheck {
    /// The moves on offer, as read when the check was built.
    moves: Vec<MoveOffered>,
}

impl MovesCheck {
    /// A check naming `moves`.
    #[must_use]
    pub const fn new(moves: Vec<MoveOffered>) -> Self {
        Self { moves }
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
            verdict(&self.moves),
        )]
    }
}

/// Pass where nothing is on offer, and a warning naming each move otherwise.
fn verdict(moves: &[MoveOffered]) -> Verdict {
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
