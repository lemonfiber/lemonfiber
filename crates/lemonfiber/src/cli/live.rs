//! Which channel to open for a member, and which title to play in their browser.

use clap::Args;

/// A channel to open, or a live stream to keep or close, as the command line spells it.
#[derive(Debug, Args)]
pub struct RawTune {
    /// The channel, by the id the channels list gives it.
    pub channel: Option<String>,
    /// Who watches it, named the way they sign in.
    #[arg(long)]
    pub member: Option<String>,
    /// The live stream to keep or close, as an earlier answer named it.
    #[arg(long)]
    pub stream: Option<String>,
    /// Close the live stream named rather than keep it.
    #[arg(long)]
    pub stop: bool,
}

/// A title to play in a member's browser, as the command line spells it.
#[derive(Debug, Args)]
pub struct RawPlay {
    /// The title, by the id the shelf lists it under.
    pub item: String,
    /// Who it plays as, named the way they sign in.
    #[arg(long)]
    pub member: Option<String>,
    /// The id the browser keeps for itself: eight to sixty-four letters, digits and
    /// dashes.
    #[arg(long)]
    pub device: Option<String>,
    /// The pass the browser holds, kept for longer around the same session.
    #[arg(long)]
    pub pass: Option<String>,
}
