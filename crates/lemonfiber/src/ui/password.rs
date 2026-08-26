//! Setting the password this surface asks for.
//!
//! Asked for at the keyboard rather than taken from the command line. A password
//! given as an argument is in the shell's history and in the list of processes this
//! machine is running, and both outlive the moment it was typed.
//!
//! Asked for twice, because the whole value of the thing is that nothing here can
//! read it back: a password mistyped once is not a password anybody can be told
//! about afterwards, and comparing two answers is the only check available to a
//! store that keeps no answer.
//!
//! The words are here and the printing is at the edge, which is the arrangement the
//! surface beside this already makes: what an operator is told is proven rather than
//! demonstrated.

use std::path::Path;

use lemonfiber_core::admission::{credential, Credential};
use lemonfiber_core::config::store::Failure;
use lemonfiber_core::error::{Code, Diagnose as _, Problem, Remedy, Severity, State};
use lemonfiber_core::ports::random::Random;
use lemonfiber_core::PRODUCT;

use crate::prompt::Answers;

/// Raised when the two answers were not the same word.
const MISTYPED: Code = Code::new("ADMIT-3");

/// What is asked above the first answer.
const ASKS: &str = "A password for the web interface:";

/// What is asked above the second.
const AGAIN: &str = "The same password again:";

/// Set it, and say what became of it.
///
/// # Errors
///
/// Returns the [`Problem`] to report where there is nowhere to keep a password, where
/// the two answers differed, where the password is too short to be one, or where the
/// file could not be written. Boxed for the reason every other problem on this path is
/// boxed: a result carrying all of that inline on the way that succeeds pays for the
/// failure on every call.
pub(crate) fn set(
    answers: &dyn Answers,
    random: &dyn Random,
    kept: Option<&Path>,
) -> Result<Vec<String>, Box<Problem>> {
    let Some(path) = kept else {
        return Err(Box::new(Failure::Nowhere.problem()));
    };
    let chosen = answers.secret(ASKS);
    if chosen != answers.secret(AGAIN) {
        return Err(Box::new(mistyped()));
    }
    let credential = Credential::set(&chosen, random).map_err(|weak| Box::new(weak.problem()))?;
    credential::keep(path, &credential).map_err(|failure| Box::new(failure.problem()))?;
    Ok(said())
}

/// What setting one says, in order.
fn said() -> Vec<String> {
    vec![
        format!("A password is now set for {PRODUCT}'s web interface."),
        String::new(),
        "It is kept as something that proves an answer right, not as the answer. This machine \
         can tell a right password from a wrong one and cannot tell you what the right one is, \
         and neither can anybody who takes the file."
            .to_owned(),
        "Forgetting it means setting another one, not recovering this one.".to_owned(),
    ]
}

/// The two answers were not the same word.
fn mistyped() -> Problem {
    Problem::new(
        MISTYPED,
        Severity::Error,
        "Those two passwords were not the same",
        "Nothing was changed. The password is asked for twice because nothing here can read \
         one back afterwards, so the second answer is the only check there is that the first \
         one was typed the way it was meant.",
        Remedy::new("Ask for it again"),
    )
    .in_state(State::Guided)
}
