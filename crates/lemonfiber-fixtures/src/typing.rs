//! Somebody at a keyboard answering whatever an act asks, or nobody there.

use std::sync::{Arc, Mutex};

use lemonfiber_ports::asking::Asking;

/// Somebody at a keyboard answering each question in turn, or nobody there, noting each
/// question and whether it was asked as a secret.
pub struct Typing {
    /// Whether anyone is there to answer.
    present: bool,
    /// What is typed for each question in turn, the last for every one after.
    answers: Vec<String>,
    /// Every question, and whether it was asked without showing what is typed.
    asked: Mutex<Vec<(String, bool)>>,
}

impl Typing {
    /// Somebody typing `answer` to every question.
    #[must_use]
    pub fn answering(answer: &'static str) -> Arc<Self> {
        Self::answering_each(vec![answer.to_owned()])
    }

    /// Somebody typing `answers` to the questions in turn, and the last to every one after.
    #[must_use]
    pub fn answering_each(answers: Vec<String>) -> Arc<Self> {
        Arc::new(Self {
            present: true,
            answers,
            asked: Mutex::new(Vec::new()),
        })
    }

    /// Nobody at a keyboard: every question is answered with nothing, and still noted,
    /// so a test can say no question was put.
    #[must_use]
    pub fn nobody() -> Arc<Self> {
        Arc::new(Self {
            present: false,
            answers: Vec::new(),
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
        let turn = self.asked().len();
        crate::noted(&self.asked, (question.to_owned(), secret));
        self.answers
            .get(turn)
            .or_else(|| self.answers.last())
            .cloned()
            .unwrap_or_default()
    }
}

impl Asking for Typing {
    fn present(&self) -> bool {
        self.present
    }

    fn ask(&self, question: &str) -> String {
        self.typed(question, false)
    }

    fn secret(&self, question: &str) -> String {
        self.typed(question, true)
    }
}
