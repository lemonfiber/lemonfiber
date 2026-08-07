//! The one place this binary reads from a person.
//!
//! A file of its own, and a very small one, because it is the part of asking a
//! question that no test can stand in for: a real terminal, real standard input,
//! and the terminal's echo suppressed for a secret. Everything about *what* to
//! ask and what an answer means is in [`crate::prompt`], where it can be proven
//! against a script; what is here is the wire to the human, and nothing else.

use std::io::Write as _;

use crate::prompt::Answers;

/// Answers typed by whoever is running this.
pub(crate) struct Keyboard;

impl Answers for Keyboard {
    fn ask(&self, question: &str) -> String {
        print!("{question} ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        line.trim().to_owned()
    }

    /// Read a secret without echoing it, its surrounding whitespace trimmed.
    ///
    /// A password shown as it is typed reaches scrollback and any shoulder or
    /// screen recording, so it is read with the terminal's echo suppressed. The
    /// trim is the same the other fields get: a pasted key's stray newline is the
    /// common error, and removing it serves the operator.
    fn secret(&self, prompt: &str) -> String {
        rpassword::prompt_password(format!("{prompt} "))
            .unwrap_or_default()
            .trim()
            .to_owned()
    }
}
