//! What the command line accepts, and nothing about what it means.
//!
//! Declaration only: the shape of every subcommand and flag, kept apart from the
//! dispatcher that routes them and the translation that turns them into the core's
//! own commands. A flag is added here; what it does is added next door.

mod allowance;
mod bandwidth;
mod credentials;
mod plugin;
mod removing;
mod repair;
mod serving;
mod setup;

use std::path::PathBuf;

use clap::{CommandFactory, Parser};

mod request;
mod under;
mod wiring;

pub use under::{
    AlertCommand, ConfigAction, HostingCommand, HouseholdCommand, Kept, MigrateCommand,
    QualityCommand, UpdateCommand,
};

// Re-exported rather than reached for through the module they now live in: where a
// flag is declared is this file's business and nobody else's, and moving one would
// otherwise be a change at every call site that names it.
pub use allowance::{RawAllowance, RawUnrated};
pub use bandwidth::RawBandwidth;
pub use credentials::RawCredentials;
pub use plugin::{Authoring, PluginCommand};
pub use removing::{RawRemoval, RawRemoving};
pub use repair::{Fixing, Mending, RawDoctor};
pub use serving::{Asked, RawUi};
pub use setup::RawSetup;

// Re-exported so that `cli::Request` still names it: where the subcommands are
// written down is this file's business, and moving them would otherwise be a change
// at every call site that matches on one.
pub use request::Request;
pub use wiring::WiringCommand;

/// Set up and run your media stack.
#[derive(Debug, Parser)]
#[command(name = "lemonfiber", version, about)]
pub struct Cli {
    /// Print machine-readable output.
    #[arg(long, global = true)]
    pub json: bool,

    /// Say what would happen, and change nothing.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Take the stack from a run that claimed it and did not give it back.
    #[arg(long, global = true)]
    pub force: bool,

    /// Operate a stack directory of your own instead of the built-in one.
    #[arg(long, global = true, value_name = "PATH")]
    pub stack_dir: Option<PathBuf>,

    /// Keep lemonfiber's own configuration under a directory of your own.
    #[arg(long, global = true, value_name = "PATH")]
    pub config_dir: Option<PathBuf>,

    /// Keep lemonfiber's own data under a directory of your own.
    #[arg(long, global = true, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,

    /// What was asked for, or nothing at all — which is the terminal interface.
    #[command(subcommand)]
    pub command: Option<Request>,
}

/// What this binary can do, as clap renders it.
///
/// Here rather than at the edge because it is a property of the parser, and the
/// one place that prints it should not also be the place that knows how.
#[must_use]
pub fn help() -> String {
    Cli::command().render_long_help().to_string()
}

#[cfg(test)]
mod tests;
