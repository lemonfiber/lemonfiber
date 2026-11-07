//! The phones that asked to be woken, as the command line asks about them.

use clap::{Args, Subcommand};

/// What `push-endpoint` was asked for.
#[derive(Debug, Args)]
pub struct RawPushEndpoint {
    /// Which of the two to do.
    #[command(subcommand)]
    pub action: PushEndpointCommand,
}

/// The two things to do about a push endpoint here: a phone registers its own.
#[derive(Debug, Subcommand)]
pub enum PushEndpointCommand {
    /// List every push endpoint by name, address and when it was registered.
    List,
    /// Remove a push endpoint by name. Its phone is not woken from then on.
    Remove {
        /// The name it was registered under.
        name: String,
    },
}
