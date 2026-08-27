//! The setup wizard's flags, exactly as the command line gives them.
//!
//! Apart from the parser for room rather than for principle: setup takes more
//! flags than every other command put together, and a declaration that long sits
//! between the root command and the subcommand tree it belongs to.

use std::path::PathBuf;

/// The setup flags exactly as the command line gives them, before they are typed
/// and checked — a plain carrier so the parse is one argument, not a dozen.
///
/// This is also the command line's own declaration of them, flattened into the
/// subcommand: one list of flags rather than a definition here and a copy there,
/// which is a copy that only ever falls out of step in one direction.
#[derive(Debug, Default, clap::Args)]
pub struct RawSetup {
    /// Report where setup stands and ask nothing. Takes precedence over the
    /// answers below, so a run that asks where it is never also answers.
    #[arg(long)]
    pub status: bool,
    /// Apply without a prompt to confirm — required for an unattended run.
    #[arg(long)]
    pub yes: bool,
    /// How to fetch content: `both`, `usenet`, `torrent`, or `none`.
    #[arg(long, value_name = "PROTOCOLS")]
    pub protocols: Option<String>,
    /// Where the library and downloads live.
    #[arg(long, value_name = "PATH")]
    pub data_location: Option<PathBuf>,
    /// An indexer's API base URL.
    #[arg(long, value_name = "URL")]
    pub indexer_url: Option<String>,
    /// The indexer's API key.
    #[arg(long, value_name = "KEY")]
    pub indexer_key: Option<String>,
    /// The Usenet provider's hostname.
    #[arg(long, value_name = "HOST")]
    pub usenet_host: Option<String>,
    /// The port the Usenet provider answers on (defaults to 563).
    #[arg(long, value_name = "PORT")]
    pub usenet_port: Option<u16>,
    /// The Usenet account username.
    #[arg(long, value_name = "USER")]
    pub usenet_user: Option<String>,
    /// The Usenet account password.
    #[arg(long, value_name = "PASS")]
    pub usenet_pass: Option<String>,
    /// Whether the Usenet connection uses TLS (defaults to yes).
    #[arg(long, value_name = "BOOL")]
    pub usenet_tls: Option<bool>,
    /// How to serve the library: `docker`, `native`, or `none`.
    #[arg(long, value_name = "MODE")]
    pub library: Option<String>,
    /// The container user, as `UID:GID`.
    #[arg(long, value_name = "UID:GID")]
    pub service_user: Option<String>,
    /// Whether a VPN carries the torrent traffic. Where torrents are chosen and
    /// this is absent, the run proceeds unprotected and records that it did.
    #[arg(long, value_name = "BOOL")]
    pub vpn: Option<bool>,
    /// Whether others in the home will use it.
    #[arg(long, value_name = "BOOL")]
    pub household: Option<bool>,
    /// What to be told about: `problems`, `completions`, or `everything`.
    #[arg(long, value_name = "APPETITE")]
    pub notifications: Option<String>,
    /// Whether to start the stack when the machine boots.
    #[arg(long, value_name = "BOOL")]
    pub autostart: Option<bool>,
}
