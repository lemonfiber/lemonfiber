//! What a trace was asked to follow, and how it is written.
//!
//! Its own file for the reason the removal flags have one: a request with a term and
//! two flags is a declaration of its own, and the file that holds every subcommand has
//! a cap on it that this would have crossed.

use clap::Args;

/// What to follow, and how far a trace may go to find it.
#[derive(Debug, Args)]
pub struct RawTrace {
    /// The show or film to follow, named as you would say it.
    #[arg(required = true)]
    pub term: Vec<String>,
    /// Narrow to one season, instead of every season of the show.
    #[arg(long)]
    pub season: Option<u32>,
    /// Ask the indexers what they carry, to tell "nothing at your quality" from
    /// "nothing at all". Spends one real search against their daily allowance.
    #[arg(long)]
    pub search: bool,
}
