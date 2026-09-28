//! Pairing a phone with this stack, as the command line asks for it.

use clap::{Args, Subcommand};

/// What `companion` was asked for.
#[derive(Debug, Args)]
pub struct RawCompanion {
    /// Which of the two to do.
    #[command(subcommand)]
    pub action: CompanionCommand,
}

/// The two things to do about a phone.
#[derive(Debug, Subcommand)]
pub enum CompanionCommand {
    /// Make pairing material for a phone: a code for its camera and the same text to type.
    ///
    /// It names the address the phone reaches, the certificate that address presents,
    /// when the material stops being good, and this stack's own identifier. It carries
    /// no password, and scanning it lets nobody in: the phone still signs in with
    /// yours. Needs the web interface served encrypted on your network, on a port that
    /// stays the same — `lemonfiber ui --lan --tls --port <port>`.
    Pair,
    /// Replace the certificate the web interface presents to a paired phone.
    ///
    /// Every phone paired with this machine refuses it afterwards, as it should for a
    /// certificate it was never shown, until it is paired again. So on its own this
    /// says that and replaces nothing; `--confirm` replaces it.
    Certificate {
        /// Replace it, having been told what replacing it costs.
        #[arg(long)]
        confirm: bool,
    },
}
