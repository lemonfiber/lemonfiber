//! How much of a member's shelf to show.

use clap::Args;

/// How much of a shelf to show, or which part of it.
#[derive(Debug, Clone, Default, Args)]
pub struct RawShelf {
    /// How many to show, newest first.
    #[arg(long)]
    pub most: Option<u32>,
    /// Show one title instead: what it is, its seasons and episodes, and where it is
    /// served — named by the id the shelf lists it under.
    #[arg(long, conflicts_with_all = ["most", "part_way"])]
    pub title: Option<String>,
    /// Show what they were part-way through instead, and how far.
    #[arg(long, conflicts_with = "defaults")]
    pub part_way: bool,
}
