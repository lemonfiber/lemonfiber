//! What one run was asked about the download queue.
//!
//! Two asks under one word. The reading is what every surface offers: what is wedged,
//! why, how many strikes it has, and what the newest runs did. A run acts on the queue,
//! and is asked for only from the terminal, where `--dry-run` says what it would do.

/// What one run was asked to do about the download queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Queueing {
    /// Say what the latest pass found and what the newest runs did.
    Read,
    /// Run one pass now: classify the queue, and act on what has struck out where
    /// healing is on.
    Heal,
}
