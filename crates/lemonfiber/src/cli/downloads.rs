//! Pausing every download client at once, as the command line asks for it.

use clap::{Args, Subcommand};

/// What `downloads` was asked for.
#[derive(Debug, Args)]
pub struct RawDownloads {
    /// Which of the two to do.
    #[command(subcommand)]
    pub action: DownloadsCommand,
    /// The offer being answered, as the rehearsal printed it. Carrying one, no client is
    /// told anything where the clients, or what each said it was doing, have moved.
    #[arg(long, value_name = "NAME", global = true)]
    pub offer: Option<String>,
}

/// The two things to do to every download client at once.
#[derive(Debug, Subcommand)]
pub enum DownloadsCommand {
    /// Stop every download client fetching, until you resume them.
    ///
    /// Each client is named with what it said afterwards, so one that went on fetching
    /// is named as fetching. A pause holds until you resume it: a schedule, a new month
    /// or a lifted cap does not undo it.
    Pause,
    /// Let every download client fetch again.
    Resume,
}
