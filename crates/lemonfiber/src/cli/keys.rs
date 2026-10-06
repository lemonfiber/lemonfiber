//! The keys another program reaches the web surface with, as the command line asks.

use clap::{Args, Subcommand};

/// What `key` was asked for.
#[derive(Debug, Args)]
pub struct RawKey {
    /// Which of the three to do.
    #[command(subcommand)]
    pub action: KeyCommand,
}

/// The three things to do about a key.
#[derive(Debug, Subcommand)]
pub enum KeyCommand {
    /// Mint a key for a program, under a name and with one scope.
    ///
    /// Its secret is shown once, here, beside the address and the certificate pin a
    /// program on another machine needs. Nothing keeps the secret: lose it and mint
    /// another. Every mint is journaled and raises an alert.
    Mint {
        /// What to call it, as in home-assistant.
        name: String,
        /// What it admits: read, act or member:<account>.
        #[arg(long)]
        scope: String,
        /// What it is for: home-assistant, mcp or other.
        #[arg(long)]
        purpose: String,
    },
    /// List every key by name, scope, purpose, state, minting time and last use.
    List,
    /// Revoke a key by name. It is refused from its next request.
    Revoke {
        /// The key's name, as the listing shows it.
        name: String,
    },
}
