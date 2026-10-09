//! Starting a form and tearing one down, as the command line spells them.
//!
//! Its own file for the same reason the other flag carriers have one: `cli.rs` is the
//! subcommand tree, and a struct of flags is a detail of one branch of it.

use clap::Args;

/// What to start, as the command line spells it.
#[derive(Debug, Args)]
pub struct RawUp {
    /// The forms to start; none starts everything the stack declares.
    pub forms: Vec<String>,
    /// Start only these services, leaving the rest of the form alone.
    #[arg(long = "service", value_name = "NAME")]
    pub services: Vec<String>,
    /// Start what a restart of this machine should start, and nothing otherwise.
    ///
    /// What a login runs. It brings back whichever form was last running unless
    /// you pinned one, and it declines — saying why — where you stopped the stack
    /// on purpose, where you never asked for it to start on its own, or where this
    /// machine is on its battery and you have not said to start anyway. It waits
    /// for the container engine to finish starting and tries again while the
    /// network is still arriving, and if the stack still does not come back it
    /// records that, so the next thing you type tells you once rather than not at
    /// all. Naming a form or a service alongside it is refused: which forms come
    /// back is the record's answer, not this command line's.
    #[arg(long, conflicts_with_all = ["forms", "services"])]
    pub at_boot: bool,
}

/// What to stop, and whether to wait for downloads first, as the command line spells it.
#[derive(Debug, Args)]
pub struct RawDown {
    /// The forms to stop; none stops everything the stack declares.
    pub forms: Vec<String>,
    /// Stop only these services, leaving the rest of the form running.
    #[arg(long = "service", value_name = "NAME")]
    pub services: Vec<String>,
    /// Let anything still downloading finish before stopping.
    ///
    /// Not for a stop of named services: what is in flight is a question about
    /// the download clients a form holds, so naming two services that are not
    /// download clients would wait on downloads stopping them cannot interrupt.
    #[arg(long, conflicts_with = "services")]
    pub wait: bool,
    /// Stop without asking about anything still downloading.
    #[arg(long, conflicts_with = "wait")]
    pub yes: bool,
}

/// What to restart, as the command line spells it.
#[derive(Debug, Args)]
pub struct RawRestart {
    /// The form holding them.
    pub form: String,
    /// The services to restart; none restarts the whole form.
    pub services: Vec<String>,
    /// The offer being answered, as the rehearsal printed it. A restart carrying one is
    /// refused where the services it would restart are no longer those.
    #[arg(long, value_name = "NAME")]
    pub offer: Option<String>,
}
