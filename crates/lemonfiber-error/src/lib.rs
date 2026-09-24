//! The error model every crate of the workspace reports through.
//!
//! A [`Problem`] carries what happened, what it means and what to do as fields, so a
//! problem that explains nothing cannot be constructed. Every [`Code`] one can carry
//! is declared in [`codes`]. Beside it are the three things
//! every problem's wording passes through: the withholding that keeps a credential out
//! of anything an operator is shown, how a failure that survived its retries is said,
//! and how a count is written in a sentence.
//!
//! Below `lemonfiber-ports` rather than inside it: a port reports failures in this
//! vocabulary, and the vocabulary is not itself a port.

pub mod plural;
mod problem;
pub mod retry;
pub mod withheld;

pub use problem::*;
