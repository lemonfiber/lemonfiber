//! What the media server plays now, the channels a member may watch and their guide, which
//! channel to open for them, and which title to play in their browser.

use clap::{Args, Subcommand};

/// What is watched: what the media server plays now, the channels a member is offered and
/// their guide, a channel opened for them and a title in their browser.
#[derive(Debug, Subcommand)]
pub enum Watching {
    /// Show what the media server is playing now: who is watching what, and where.
    ///
    /// Every session in the house, or one member's with `--member`.
    Playing {
        /// Whose sessions, by the name they are known by.
        #[arg(long)]
        member: Option<String>,
    },
    /// Show the live TV channels one member may watch, each with what is on it now.
    ///
    /// Read as that member: a channel whose programme now is outside their limits is left
    /// out, and a member whose account may not watch live TV is shown none.
    Channels {
        /// Whose channels, named the way they sign in.
        #[arg(long)]
        member: Option<String>,
    },
    /// Show the live TV guide one member may watch: each channel's programmes from now.
    ///
    /// Read as that member across the hours `LEMONFIBER_GUIDE_HOURS` holds: a channel whose
    /// programme now is outside their limits is left out, a programme outside them is left
    /// out as a gap, and each channel says where the media server's guide ends.
    Guide {
        /// Whose guide, named the way they sign in.
        #[arg(long)]
        member: Option<String>,
    },
    /// Open a channel's live stream for one member, and say where it is served.
    ///
    /// Each location carries a pass for that live stream and that member, which lapses
    /// five minutes after it is issued. Ask again with `--stream` before then to keep it,
    /// and with `--stop` as well to close it. Every location is also drawn as a code to
    /// point a camera at.
    Tune(RawTune),
    /// Say where one title plays in a member's browser, on a pass holding a session on
    /// their own account.
    ///
    /// The pass lapses five minutes after it is issued; hand it back with `--pass` before
    /// then to keep playing. The location is also drawn as a code to point a camera at.
    Play(RawPlay),
}

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
