//! What an invitation lets the person it is for watch, as the command line spells it.
//!
//! Its own file for the same reason the other flag carriers have one: `cli.rs` is the
//! subcommand tree, and a struct of flags is a detail of one branch of it.

use clap::{Args, ValueEnum};

/// What an invitation lets the person it is for watch, as the command line spells it.
///
/// Flattened rather than sat on the request as three fields, because they are one
/// decision taken at one moment and the core carries them as one value — and three
/// fields here would be a request the translation next door had to put back together.
#[derive(Debug, Args)]
pub struct RawAllowance {
    /// Let them watch only these libraries, named as the media server names them;
    /// none lets them watch all of them.
    #[arg(long = "library", value_name = "NAME")]
    pub libraries: Vec<String>,
    /// Hold back anything the media server rates above this age — 0, 7, 12, 15 and
    /// 18 are the steps offered; none sets no limit at all.
    #[arg(long, value_name = "AGE")]
    pub age_limit: Option<u32>,
    /// What to do about content the media server has no rating for; anybody being
    /// narrowed has it held back unless this says otherwise.
    #[arg(long, value_name = "CHOICE")]
    pub unrated: Option<RawUnrated>,
}

/// What is to happen to content the media server has no rating for.
///
/// Offered as two words rather than as a switch, because a switch has a default the
/// operator cannot see and this choice has a cost either way: holding it back makes
/// legitimate content invisible, and letting it through lets through the one thing
/// nobody rated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RawUnrated {
    /// Hold it back.
    Block,
    /// Let it through.
    Allow,
}
