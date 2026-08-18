//! Where setup's answers come from — a person, or the command line.
//!
//! The reading half of the wizard's [`Prompt`](lemonfiber_core::app::setup::Prompt)
//! port: the core decides what to ask and what each answer means, and this turns a
//! question into something asked and a typed line back. Two ways to answer, and
//! the wizard cannot tell them apart, which is what keeps them honest.

#[cfg(test)]
pub(crate) mod fixtures;

pub(crate) mod flags;
pub(crate) mod terminal;

pub(crate) use flags::{Flags, RawSetup, SetupFlags};
pub(crate) use terminal::Terminal;

/// Where the answers to these questions come from.
///
/// A person at a keyboard, in a real run. The distinction exists so that what to
/// ask, and what an answer means, can be proven against a script — the terminal
/// itself is the one part of this that no test can stand in for, and it is kept
/// behind here in [`crate::keyboard`].
/// Ask a yes-or-no question, taking the default where the answer is neither.
///
/// One implementation, because two would eventually disagree about what counts as yes —
/// and the difference between "anything but n" and "only y" is the difference between a
/// stack somebody changed by accident and one they meant to.
pub(crate) fn yes_no(answers: &dyn Answers, question: &str, default: bool) -> bool {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    match answers
        .ask(&format!("{question} {hint}:"))
        .to_lowercase()
        .as_str()
    {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    }
}

pub trait Answers {
    /// Show a question and read the trimmed answer, empty at end of input.
    fn ask(&self, question: &str) -> String;

    /// Read a secret, without it appearing as it is typed.
    fn secret(&self, prompt: &str) -> String;
}
