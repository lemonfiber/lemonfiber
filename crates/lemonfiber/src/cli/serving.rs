//! What serving the surface is asked for, and what a question about it carries.
//!
//! Both are the same shape of thing — a command taking more flags than a line of
//! the parser has room for — and both belong to the surface rather than to the
//! declaration of the tree.

use std::path::PathBuf;

use clap::Args;
use lemonfiber_core::app::bundle::LINES;

/// What the web interface was asked for.
///
/// Declared as one thing rather than as three loose values on a variant, for the
/// same reason the bundle's flags are: the same three would otherwise be written
/// out again by whoever passes them on.
#[derive(Debug, Args)]
pub struct RawUi {
    /// The port to listen on. Without it, whichever one is free is used and the
    /// whole address is printed.
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,
    /// Do not ask this desktop to open a browser.
    #[arg(long)]
    pub no_browser: bool,
    /// Serve the interface from this directory rather than from the binary.
    ///
    /// No build carries a web app of its own yet, so this is the only way to
    /// serve one. A build asked without it says as much rather than answering
    /// with an empty page.
    #[arg(long, value_name = "PATH")]
    pub assets: Option<PathBuf>,
    /// Offer this to your network, rather than to this machine only.
    ///
    /// Refused unless a password has been set. This surface can start, stop and
    /// reconfigure everything and reaches every password the system holds, so it is
    /// not offered to a network with nothing in front of it.
    #[arg(long)]
    pub lan: bool,
    /// Set the password this surface asks for, before it starts.
    ///
    /// Asked for at the keyboard and never on this line: a password typed as an
    /// argument is a password in your shell's history and in the list of processes
    /// this machine is running.
    #[arg(long)]
    pub set_password: bool,
}

/// What a support bundle was asked for.
///
/// Declared as one thing rather than as six loose values on a variant, because the same
/// six would otherwise be written out again by whoever passes them on — and a flag added
/// to one list and not the other is a flag that silently does nothing.
#[derive(Debug, Args)]
pub struct Asked {
    /// Produce the bundle, having seen what it would hold.
    #[arg(long)]
    pub write: bool,
    /// Where to write it, instead of into this directory.
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
    /// How many log lines to take from each service.
    #[arg(long, value_name = "LINES", default_value_t = LINES)]
    pub logs: u32,
    /// Include media filenames, which are replaced by default.
    #[arg(long)]
    pub filenames: bool,
    /// Show one setting as it is, named exactly as the bundle names it.
    ///
    /// Repeatable, and refused without `--confirm` on the same run: a flag that
    /// publishes a credential is not one to honour because it turned up on a
    /// command line somebody copied.
    #[arg(long, value_name = "SETTING")]
    pub reveal: Vec<String>,
    /// Confirm showing the settings named by `--reveal`.
    #[arg(long)]
    pub confirm: bool,
}
