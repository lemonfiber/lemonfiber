//! Somebody at a keyboard, answering whatever an act asks.

use std::sync::{Arc, Mutex};

use crate::ports::asking::Asking;

/// Somebody at a keyboard answering every question with one answer, noting each
/// question and whether it was asked as a secret.
pub(crate) struct Typing {
    /// What is typed for every question.
    answer: &'static str,
    /// Every question, and whether it was asked without showing what is typed.
    asked: Mutex<Vec<(String, bool)>>,
}

impl Typing {
    pub(crate) fn answering(answer: &'static str) -> Arc<Self> {
        Arc::new(Self {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn asked(&self) -> Vec<(String, bool)> {
        self.asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
    }
}

impl Asking for Typing {
    fn present(&self) -> bool {
        true
    }

    fn ask(&self, question: &str) -> String {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push((question.to_owned(), false));
        }
        self.answer.to_owned()
    }

    fn secret(&self, question: &str) -> String {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push((question.to_owned(), true));
        }
        self.answer.to_owned()
    }
}
