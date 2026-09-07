//! What the command line accepts about the credentials this stack holds.
//!
//! Three asks under one word, spelled as two optional names and a confirmation
//! rather than as subcommands, because two of the three are one word longer than
//! the reading and the reading is what people type.
//!
//! **No value is accepted here.** A replacement is either one lemonfiber mints or
//! one already changed with whoever issued it, so nothing this surface takes is
//! ever a secret — which keeps every credential in this stack out of the shell
//! history file, where it would otherwise sit in the clear for as long as that
//! file lives.

use clap::Args;

/// What is being asked about the credentials this stack holds.
#[derive(Debug, Clone, Args)]
pub struct RawCredentials {
    /// Print one credential's value. Asks for confirmation before it prints.
    #[arg(long, value_name = "NAME", conflicts_with = "rotate")]
    pub reveal: Option<String>,
    /// Replace one credential, proving the replacement against the live service
    /// before the existing value stops being the one in force.
    #[arg(long, value_name = "NAME")]
    pub rotate: Option<String>,
    /// Yes, print it in the clear — having read what that costs.
    #[arg(long, requires = "reveal")]
    pub confirm: bool,
}
