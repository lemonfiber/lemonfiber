//! Where the notification back-ends are kept: what each is and what became of its
//! deliveries, and apart from that the secret each is reached with.

use std::path::PathBuf;

use super::{Paths, NOTIFY_BACKENDS, NOTIFY_SECRETS};

impl Paths {
    /// Every notification back-end the stack delivers to, with how far each is proven and
    /// what became of its deliveries.
    #[must_use]
    pub fn notify_backends(&self) -> PathBuf {
        self.config.join(NOTIFY_BACKENDS)
    }

    /// The secret each notification back-end is reached with, apart from the rest so
    /// nothing that lists the back-ends ever reads one.
    #[must_use]
    pub fn notify_secrets(&self) -> PathBuf {
        self.config.join(NOTIFY_SECRETS)
    }
}
