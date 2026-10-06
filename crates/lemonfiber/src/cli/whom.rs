//! Whom a household word is narrowed to, as the command line spells it.
//!
//! Its own file for the same reason the other flag carriers have one: `cli.rs` is the
//! subcommand tree, and a struct of flags is a detail of one branch of it.

use clap::Args;

/// Whom a household word is narrowed to: one member, or the household's defaults.
///
/// Flattened onto both words that narrow, because it is one choice and the core carries
/// it as one value. The two flags refuse each other: the defaults are nobody, and a run
/// naming somebody as well has asked two things at once.
#[derive(Debug, Args)]
pub struct RawWhom {
    /// One member, named the way you would say it.
    #[arg(long)]
    pub member: Option<String>,
    /// Somebody invited with the household's defaults, read without asking about
    /// anybody's account.
    #[arg(long, conflicts_with = "member")]
    pub defaults: bool,
}
