//! What `space` and `stop-seeding` were asked for: the disk's account and its offer, and
//! one completed download let go.

use clap::Args;

/// What `space` was asked for.
#[derive(Debug, Args)]
pub struct RawSpace {
    /// The offer being answered, as the account printed it.
    #[arg(long, value_name = "NAME")]
    pub offer: Option<String>,
    /// Take only this category of what the offer costs nothing to take: `orphaned` or
    /// `extracted`. May be repeated.
    #[arg(long, value_name = "CATEGORY", requires = "offer")]
    pub only: Vec<String>,
}

/// What `stop-seeding` was asked for.
#[derive(Debug, Args)]
pub struct RawStopSeeding {
    /// The completed download, as the account and the client both name it.
    pub download: String,
    /// The offer being answered, as the run that made it printed it.
    #[arg(long, value_name = "NAME")]
    pub offer: Option<String>,
}
