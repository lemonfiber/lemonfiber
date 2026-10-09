//! A restart, and the offer it answers where it carries one.

/// What a restart was asked to restart, and the offer it answers where it carries one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restarting {
    /// The forms holding those services.
    pub forms: Vec<String>,
    /// The services to restart; empty restarts the whole form.
    pub services: Vec<String>,
    /// The offer a rehearsal answered, where the restart carries one back: refused
    /// where the services it would restart are no longer those. None acts as without
    /// one.
    pub offer: Option<String>,
}
