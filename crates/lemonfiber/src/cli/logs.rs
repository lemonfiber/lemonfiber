//! Reading what services say, as the command line spells it.
//!
//! Its own file for the same reason the other flag carriers have one: `cli.rs` is the
//! subcommand tree, and a struct of flags is a detail of one branch of it.

use clap::Args;

/// Which services to read, how much of what they said, and how to keep reading.
#[derive(Debug, Args)]
pub struct RawLogs {
    /// The services to read; none reads them all.
    pub services: Vec<String>,
    /// Read only the services a form declares.
    #[arg(long, value_name = "FORM")]
    pub form: Vec<String>,
    /// Keep reading as new lines arrive.
    #[arg(long, short)]
    pub follow: bool,
    /// Read them on a screen that can be scrolled back and filtered.
    #[arg(long, conflicts_with = "follow")]
    pub watch: bool,
    /// How many existing lines to begin with.
    #[arg(long, default_value_t = 50)]
    pub tail: u32,
}
