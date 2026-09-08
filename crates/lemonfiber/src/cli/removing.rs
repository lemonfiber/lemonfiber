//! What an uninstall was asked to remove, and how it is written.
//!
//! Its own file for the reason the line the bandwidth flags sit on is: a request with
//! a word off a list and three flags is a declaration of its own, and the file that
//! holds every subcommand has a cap on it that this would have crossed.

use clap::{Args, ValueEnum};

/// Which of the four removals an uninstall was asked for.
///
/// Spelled as four words rather than as a level, because they are four decisions and
/// not a scale: an operator who wants the containers gone has not thereby said
/// anything about their library, and a number would let one be read as implying the
/// next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RawRemoval {
    /// Remove nothing; stop the services.
    Stop,
    /// Remove the containers, their network and the images pulled for them.
    Services,
    /// Remove each service's own settings, everything lemonfiber keeps, and the
    /// credentials in both.
    Configuration,
    /// Remove the library and the downloads.
    Media,
}

/// What an uninstall was asked to remove, and what was answered about it.
///
/// Held together rather than spelled out at the request, because they are one
/// decision with parts — and because the dispatcher's table has one line per request
/// in it.
#[derive(Debug, Args)]
pub struct RawRemoving {
    /// Which removal: `stop`, `services`, `configuration` or `media`.
    #[arg(value_name = "REMOVAL")]
    pub tier: RawRemoval,
    /// Go ahead, having read what would go.
    #[arg(long)]
    pub confirm: bool,
    /// The listing being answered, as the run that printed it named it.
    #[arg(long, value_name = "NAME")]
    pub agreed: Option<String>,
    /// Let anything still coming down finish before the services stop.
    #[arg(long)]
    pub wait: bool,
}
