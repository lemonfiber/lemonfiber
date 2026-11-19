//! What a request to pause or resume the download clients was given.

/// Pause every download client the stack runs, or let them all fetch again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloading {
    /// Which of the two.
    pub asked: crate::bandwidth::Pausing,
    /// The offer a rehearsal answered, where the request carries one back: no client
    /// is told anything where the clients, or what each said it was doing, have
    /// moved. None acts as without one.
    pub offer: Option<String>,
}
