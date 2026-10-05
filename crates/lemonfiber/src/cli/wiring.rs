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
    /// Named on its own it says what the change would come to — what fills the
    /// capability now, what asks for it, and what it would leave with nothing
    /// filling it — changes nothing, and prints a name for that offer; answering with
    /// that name is the yes. The wiring is read again first, and an answer given for
    /// a different reading is refused, naming what moved.
    Fill {
        /// The capability whose filler changes, such as `identity.source`.
        capability: String,
        /// The service to fill it.
        service: String,
        /// Why you chose it, recorded with the choice and read back beside it.
        #[arg(long, value_name = "TEXT")]
        reason: Option<String>,
        /// The offer being answered, as the run that made it printed it.
        #[arg(long, value_name = "NAME")]
        offer: Option<String>,
    },
}
