//! Somebody at a keyboard, answering whatever an act asks.

use std::sync::{Arc, Mutex};

use lemonfiber_ports::asking::Asking;

/// Somebody at a keyboard answering every question with one answer, noting each
/// question and whether it was asked as a secret.
pub struct Typing {
    /// What is typed for every question.
    answer: &'static str,
    /// Every question, and whether it was asked without showing what is typed.
    asked: Mutex<Vec<(String, bool)>>,
}

impl Typing {
    /// Somebody typing `answer` to every question.
    #[must_use]
    pub fn answering(answer: &'static str) -> Arc<Self> {
        Arc::new(Self {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }

    /// Every question asked so far, and whether it was asked as a secret.
    #[must_use]
    pub fn asked(&self) -> Vec<(String, bool)> {
        self.asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
    }

    /// Note `question`, and answer it.
    fn typed(&self, question: &str, secret: bool) -> String {
        crate::noted(&self.asked, (question.to_owned(), secret));
        self.answer.to_owned()
    }
}

impl Asking for Typing {
    fn present(&self) -> bool {
        true
    }

    fn ask(&self, question: &str) -> String {
        self.typed(question, false)
    }

    fn secret(&self, question: &str) -> String {
        self.typed(question, true)
    }
}
