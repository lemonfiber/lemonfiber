//! Asking the person running a command for a value, while it runs.
//!
//! Some values only the operator has, and some acts cannot go ahead without one: a
//! code a service hands out, a password of an account that is not the stack's. The
//! logic above this boundary knows what it needs and in what words to ask for it, and
//! has no terminal; a surface that has somebody at a keyboard says so and does the
//! asking, and one that has nobody, or a client on the other end of a request, says
//! nobody is there, and the act refuses naming what it needed.

/// Where a command asks the operator for a value.
pub trait Asking: Send + Sync {
    /// Whether anyone is there to answer.
    fn present(&self) -> bool;

    /// Show a question and read the answer, its surrounding whitespace trimmed, empty
    /// where nothing was given.
    fn ask(&self, question: &str) -> String;

    /// Ask for a value without it appearing as it is typed, trimmed the same way.
    fn secret(&self, question: &str) -> String;
}

/// Nobody to ask: what a run without a person at a keyboard asks through.
///
/// A value rather than an absence for the reason [`crate::narration::Silent`] is one:
/// the act asks once, in one place, whoever is or is not there.
pub struct Nobody;

impl Asking for Nobody {
    fn present(&self) -> bool {
        false
    }

    fn ask(&self, _question: &str) -> String {
        String::new()
    }

    fn secret(&self, _question: &str) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests;
