//! The lists three of this command line's words open.
//!
//! Apart from the requests themselves because they answer a different question: that
//! file says what may be asked for, and this says what the three words with verbs
//! under them accept. Each of these grows every time one of those words gains
//! something to do, which is what pushed the two apart.
//!
//! They are re-exported beside the requests, so `cli::QualityCommand` reads the same
//! as it always did.

use clap::Subcommand;

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
