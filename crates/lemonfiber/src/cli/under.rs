//! The lists three of this command line's words open.
//!
//! Apart from the requests themselves because they answer a different question: that
//! file says what may be asked for, and this says what the three words with verbs
//! under them accept. Each of these grows every time one of those words gains
//! something to do, which is what pushed the two apart.
//!
//! They are re-exported beside the requests, so `cli::QualityCommand` reads the same
//! as it always did.

use clap::{Subcommand, ValueEnum};

/// What can be decided about what the household may ask for.
///
/// Three, and they are two different errands. Choosing a policy settles what happens to
/// everything asked for from now on; approving and declining settle one thing somebody
/// has already asked for, which is why each names a request and the choice does not.
#[derive(Debug, Subcommand)]
pub enum HouseholdCommand {
    /// Choose what happens to what the household asks for, and how much it may ask.
    ///
    /// Naming only a limit leaves the policy alone, and naming only a policy leaves the
    /// limit alone — saying nothing about something is not choosing it.
    ///
    /// Television is counted a season at a time, because that is how the request service
    /// counts it: one ask for a six-season series spends six.
    Allow {
        /// Set it for one person instead of for the whole household.
        #[arg(long)]
        member: Option<String>,
        /// What happens to a request: trusted, within-a-limit, or everything-waits.
        #[arg(long)]
        policy: Option<String>,
        /// How many requests a period allows. Needs `--days` beside it.
        #[arg(long, requires = "days")]
        requests: Option<u32>,
        /// How long that period is, in days. Needs `--requests` beside it.
        #[arg(long, requires = "requests")]
        days: Option<u32>,
    },
    /// Let one waiting request through, by the number the household list gives it.
    ///
    /// Refused where there is no room left on the disk, and said as the disk rather
    /// than as anybody's limit — raising a limit would change nothing.
    Approve {
        /// The request, by the number beside it.
        request: i64,
    },
    /// Turn one waiting request down, saying why.
    ///
    /// The reason is required and it does not travel: the request service tells whoever
    /// asked that it was declined and carries no reason with it, so what you write here
    /// is yours to pass on.
    Decline {
        /// The request, by the number beside it.
        request: i64,
        /// Why, in a few words.
        #[arg(long, required = true)]
        reason: String,
    },
    /// Close the requests nobody has ruled on, once they have waited too long.
    ///
    /// Nothing is closed until you say how long is too long, and there is no period this
    /// chooses for you. Naming one records it and stops: the household is told about it,
    /// on the list you read and in the message each member is handed, before anything is
    /// closed.
    ///
    /// Run with nothing named, it does the closing — and it holds this terminal until you
    /// stop it or arrange something else, because lemonfiber starts nothing by itself.
    /// Whoever asked is told why, at the address they already gave the request service.
    Expiring {
        /// How many days a request may wait before it is closed. Records it and stops.
        #[arg(long, conflicts_with = "never")]
        after: Option<u32>,
        /// Stop closing anything for waiting, whatever was arranged before.
        #[arg(long)]
        never: bool,
    },
}

/// What to do about a setup already on this machine.
#[derive(Debug, Subcommand)]
pub enum MigrateCommand {
    /// Take over the setup already here, so lemonfiber manages it.
    ///
    /// Without `--confirm` it says what adopting would come to and writes nothing —
    /// which databases a newer version would upgrade, and where their data sits so it
    /// can be backed up first.
    Adopt {
        /// Go ahead, having backed up the data named.
        #[arg(long)]
        confirm: bool,
    },
}

/// What to do with what you are told about.
#[derive(Debug, Subcommand)]
pub enum AlertCommand {
    /// Show what you are told about, and what else you could be.
    Show,
    /// Choose how much to be told.
    Set {
        /// How much to hear: problems-only, with-completions, or everything.
        preset: String,
    },
}

/// What to do with settings.
#[derive(Debug, Subcommand)]
pub enum QualityCommand {
    /// Show the quality choice in force, and what each preset means and costs.
    Show,
    /// Choose a preset — for everything, or for one media type.
    Set {
        /// The preset: space-saving, balanced, high-quality, or maximum.
        preset: String,
        /// Apply it to one media type (tv or movies) rather than everything.
        #[arg(long = "for", value_name = "MEDIA_TYPE")]
        media_type: Option<String>,
        /// Confirm a choice this machine would have to transcode in software.
        #[arg(long)]
        confirm: bool,
    },
    /// Re-assert the recorded preset over a Recyclarr config you have hand-edited.
    ///
    /// An ordinary run keeps your edits; this is the explicit consent to let the
    /// preset win instead.
    Reapply,
    /// Upgrade existing content to the chosen quality — re-download what is already
    /// here at the higher quality.
    ///
    /// A large, bandwidth-expensive operation, separate from a preset change (which
    /// only affects future acquisitions). States the cost and does nothing until
    /// `--confirm`.
    Upgrade {
        /// Go ahead and trigger the re-search, having seen the cost.
        #[arg(long)]
        confirm: bool,
    },
}

/// What to do with one setting, or with all of them.
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Read one setting.
    Get {
        /// The setting to read.
        key: String,
    },
    /// Change one setting.
    Set {
        /// The setting to change.
        key: String,
        /// What to change it to.
        value: String,
    },
    /// Show every setting, with credentials withheld.
    Show,
}

/// Which of the two commands that outlive the request that started them.
///
/// A closed list rather than a name typed, because a word that names none of them
/// is a mistake worth catching where it was typed rather than three layers in.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Kept {
    /// The guard on the data location.
    Watch,
    /// The clock that closes requests nobody has ruled on.
    Expiring,
}

/// What can be done about what this machine keeps running.
///
/// Two, and they are the same errand in both directions. Neither is reachable from
/// anywhere else: something that starts at every login is asked for, never arrived
/// at as a side effect of the afternoon's work.
#[derive(Debug, Subcommand)]
pub enum HostingCommand {
    /// Have this machine keep one of them running, now and after a restart.
    ///
    /// Installs it into your own account — no administrator rights, and nothing
    /// another account on this machine inherits. It starts it as well as installing
    /// it, and says so, along with where a command with no terminal writes what it
    /// would have said in one.
    Install {
        /// Which one: the guard on the data location, or the clock on requests.
        what: Kept,
        /// The forms the guard stops if the data location is lost. The guard alone
        /// takes them, and it will not be installed without them.
        forms: Vec<String>,
    },
    /// Take one back off this machine, leaving nothing behind.
    ///
    /// The service definition, its registration, and its place in your login items.
    /// Asked about one that is not installed, it says so rather than failing.
    Remove {
        /// Which one.
        what: Kept,
    },
}
