//! The back-ends the stack delivers its alerts to, as the command line configures them.
//!
//! A back-end's secret is read from the environment variable named, never from the
//! command line itself, so no token sits in the shell's history.

use clap::{Args, Subcommand, ValueEnum};

/// What `notify` was asked for.
#[derive(Debug, Args)]
pub struct RawNotify {
    /// Which of the five to do.
    #[command(subcommand)]
    pub action: NotifyCommand,
}

/// What a back-end speaks, as the command line names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum NotifyKind {
    /// An ntfy server's topic.
    Ntfy,
    /// A gotify server's application.
    Gotify,
    /// An address posted each message as JSON.
    Webhook,
    /// An Apprise API address.
    Apprise,
}

/// What a back-end is reached with, each read from the environment variable named.
#[derive(Debug, Clone, Default, Args)]
pub struct NotifySecrets {
    /// The environment variable holding what it is posted with: an ntfy access token or
    /// `user:password`, a gotify application token, or a webhook's bearer token.
    #[arg(long, value_name = "VARIABLE")]
    pub token_env: Option<String>,
    /// The environment variable holding a gotify client token, which its history is read
    /// with to see a message arrive.
    #[arg(long, value_name = "VARIABLE")]
    pub reader_env: Option<String>,
}

/// How a failed delivery is tried again.
#[derive(Debug, Clone, Default, Args)]
pub struct NotifyRetries {
    /// How many attempts a delivery is given, five where none is named.
    #[arg(long, value_name = "COUNT")]
    pub attempts: Option<u32>,
    /// The minutes waited before each attempt after the first, as in 1,5,15,60,360.
    #[arg(long, value_name = "MINUTES", value_delimiter = ',')]
    pub delays: Option<Vec<u64>>,
}

/// The five things to do about the back-ends.
#[derive(Debug, Subcommand)]
pub enum NotifyCommand {
    /// List every back-end, where it reaches, whether it is proven and what became of its
    /// newest delivery.
    List,
    /// Add a back-end. It delivers nothing until `notify test` proves it.
    Add {
        /// What to call it, as in home-ntfy.
        id: String,
        /// What it speaks.
        #[arg(long, value_enum)]
        kind: NotifyKind,
        /// The server's address, with no login in it.
        #[arg(long)]
        url: String,
        /// The ntfy topic it posts to.
        #[arg(long)]
        topic: Option<String>,
        /// What it is reached with.
        #[command(flatten)]
        secrets: NotifySecrets,
        /// Accept that it is a hosted service, which what is delivered to it leaves for.
        #[arg(long)]
        hosted: bool,
        /// How a failed delivery is tried again.
        #[command(flatten)]
        retries: NotifyRetries,
    },
    /// Change what a back-end is reached with, how it retries, or whether the household
    /// may receive on it. A changed secret holds it until `notify test` proves it again.
    Set {
        /// The back-end, as the listing names it.
        id: String,
        /// What it is reached with from now on.
        #[command(flatten)]
        secrets: NotifySecrets,
        /// How a failed delivery is tried again from now on.
        #[command(flatten)]
        retries: NotifyRetries,
        /// Offer it to the household under this name, which is all a member is shown.
        #[arg(long, value_name = "NAME", conflicts_with = "no_members")]
        members: Option<String>,
        /// Withdraw it from every member who chose it.
        #[arg(long)]
        no_members: bool,
    },
    /// Send a back-end a test, and prove it where the test is seen arriving.
    Test {
        /// The back-end, as the listing names it.
        id: String,
    },
    /// Remove a back-end and forget what it is reached with.
    Remove {
        /// The back-end, as the listing names it.
        id: String,
    },
}
