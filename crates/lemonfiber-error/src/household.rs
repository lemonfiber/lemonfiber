//! What a household member's session is told when something is refused or in the way.
//!
//! A member is answered with the envelope everybody is, by the same code, and with none
//! of the operator's words: what happened and what to do come from the registry's
//! household entry for the code, and what only the operator can read is left out. The
//! choice is made here and nowhere else, so a refusal and an obstacle a member meets are
//! worded by one rule whichever route or stream they arrive on.

use serde::Serialize;

use crate::codes::{Declared, Household};
use crate::{Code, Problem, Remedy};

/// What a fault of the stack is said as to the household.
pub const FAULT: &str = "This isn't working right now, and whoever runs the house has been told.";

/// What a household member is told to do about a fault of the stack.
pub const FAULT_REMEDY: &str = "Try again a little later.";

/// What a household member is told about one code: a sentence and a remedy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Words {
    /// What happened, in the household's words.
    pub said: &'static str,
    /// What the member can do about it.
    pub remedy: &'static str,
    /// Whether it is a fault of the stack rather than anything the member did or can change.
    pub fault: bool,
}

impl Words {
    /// What the household is told about `code`: its own entry, or the fault's words where
    /// it is a fault of the stack or nothing declares it.
    #[must_use]
    pub fn of(code: Code) -> Self {
        match code.declaration().map(Declared::household) {
            Some(Household::Said { said, remedy }) => Self {
                said,
                remedy,
                fault: false,
            },
            Some(Household::Fault) | None => Self {
                said: FAULT,
                remedy: FAULT_REMEDY,
                fault: true,
            },
        }
    }
}

/// `problem` as a household member's session is answered with it.
///
/// The same code, severity and standing, with the household sentence as what happened
/// and what it means, the household remedy as the one thing to do, and the detail, the
/// cause and the steps left out: each is written for the operator, and a service's own
/// words are quoted in the first.
#[must_use]
pub fn spoken(problem: &Problem) -> Problem {
    let words = Words::of(problem.code);
    Problem {
        code: problem.code,
        severity: problem.severity,
        state: problem.state,
        summary: words.said.to_owned(),
        meaning: words.said.to_owned(),
        remedies: vec![Remedy::new(words.remedy)],
        detail: None,
        cause: None,
        steps: Vec::new(),
    }
}

/// Something in the way of one part of an answer, said by its code.
///
/// What a read carries beside the part it could not give, rather than a sentence of its
/// own: a sentence carries no code, so nothing could word it again for whoever reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Obstacle {
    /// The code that names what is in the way.
    pub code: Code,
    /// What is in the way, in one sentence.
    pub said: String,
    /// What to do about it.
    pub remedy: String,
}

impl Obstacle {
    /// What is in the way under `code`, said as `said`, with the remedy its code is
    /// declared with.
    #[must_use]
    pub fn new(code: Code, said: impl Into<String>) -> Self {
        Self {
            code,
            said: said.into(),
            remedy: code
                .declaration()
                .map_or_else(String::new, |declared| declared.remedy().to_owned()),
        }
    }

    /// The obstacle as a household member's session is told it.
    #[must_use]
    pub fn spoken(&self) -> Self {
        let words = Words::of(self.code);
        Self {
            code: self.code,
            said: words.said.to_owned(),
            remedy: words.remedy.to_owned(),
        }
    }
}

impl std::fmt::Display for Obstacle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.said, self.code)
    }
}

#[cfg(any(test, feature = "testing"))]
mod unhoused;
#[cfg(any(test, feature = "testing"))]
pub use unhoused::unhoused;

#[cfg(test)]
mod tests;
