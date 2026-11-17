//! Where what this machine does on its own is kept: whether the operator asked for the
//! stack to come back after a restart, which restart was last acted on, and what each
//! run-and-exit service's runs left.

use std::path::PathBuf;

use super::{Paths, AUTOSTART, BOOT, SCHEDULED};

impl Paths {
    /// The operator's autostart answer: whether they asked for the stack to come
    /// back after this machine restarts. Kept with configuration rather than as a
    /// setting in the environment file, because the environment file is handed to
    /// Compose as it stands and this is not a thing Compose has any use for — and
    /// kept at all because an answer gathered at setup and then discarded leaves
    /// the operator believing they chose something they did not.
    #[must_use]
    pub fn autostart(&self) -> PathBuf {
        self.config.join(AUTOSTART)
    }

    /// Which restart of this machine lemonfiber last acted on.
    ///
    /// The observation half of autostart, kept apart from the answer beside it on
    /// purpose: one records what the operator asked for and the other records what
    /// this machine did, and a single file holding both is the `enabled-unverified`
    /// trap committed to disk — a record that cannot tell wanting from working.
    ///
    /// Kept with configuration rather than beside the stack because losing it is not
    /// free: a run that forgot which restart it had already acted on would verify the
    /// same boot again, and report a failure the operator was told about a week ago as
    /// though it had just happened.
    #[must_use]
    pub fn boot(&self) -> PathBuf {
        self.config.join(BOOT)
    }

    /// What each run-and-exit service's runs left, one record per service. Kept with
    /// configuration because it is the only record of when a service last ran and how it
    /// ended, which is what tells a missed run from one that never came due.
    #[must_use]
    pub fn scheduled(&self) -> PathBuf {
        self.config.join(SCHEDULED)
    }
}
