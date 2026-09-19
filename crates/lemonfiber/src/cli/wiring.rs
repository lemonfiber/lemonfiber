//! The one verb the wiring surface has.
//!
//! Its own file rather than a line in the request list, because a read and a
//! write sharing a command line is how somebody changes a stack while meaning to
//! look at one — and the sentence that says so is more than an index of requests
//! should carry.

use clap::Subcommand;

/// Changing which service fills a capability.
#[derive(Debug, Subcommand)]
pub enum WiringCommand {
    /// Choose which service fills a capability, so everything that asked reaches it.
    ///
    /// Substituting is this and nothing else. A link asks for a capability, and
    /// which service answers is a setting — so it is recorded, it shows in the
    /// history, and `undo` puts it back.
    ///
    /// What the change would leave with nothing filling it is said before it is
    /// made. Run it with `--dry-run` to see that and write nothing.
    Fill {
        /// The capability whose filler changes, such as `identity.source`.
        capability: String,
        /// The service to fill it.
        service: String,
    },
}
