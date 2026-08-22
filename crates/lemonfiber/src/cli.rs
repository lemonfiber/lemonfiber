//! What the command line accepts, and nothing about what it means.
//!
//! Declaration only: the shape of every subcommand and flag, kept apart from the
//! dispatcher that routes them and the translation that turns them into the core's
//! own commands. A flag is added here; what it does is added next door.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use include_dir::{include_dir, Dir};
use lemonfiber_core::app::bundle::LINES;

use crate::prompt::RawSetup;

/// The stack this binary carries.
///
/// Embedding it means the common install has one thing to fetch rather than
/// two, and `build.rs` has already refused to produce this binary if the
/// manifest is one it could not read.
pub(crate) static STACK: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

/// Set up and run your media stack.
#[derive(Debug, Parser)]
#[command(name = "lemonfiber", version, about)]
pub(crate) struct Cli {
    /// Print machine-readable output.
    #[arg(long, global = true)]
    pub(crate) json: bool,

    /// Say what would happen, and change nothing.
    #[arg(long, global = true)]
    pub(crate) dry_run: bool,

    /// Take the stack from a run that claimed it and did not give it back.
    #[arg(long, global = true)]
    pub(crate) force: bool,

    /// Operate a stack directory of your own instead of the built-in one.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) stack_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Option<Request>,
}

/// What the operator asked for.
#[derive(Debug, Subcommand)]
pub(crate) enum Request {
    /// Set up the stack by answering a few questions.
    ///
    /// Interactive by default. Given the flags below, it runs unattended: each
    /// answers a question the wizard would otherwise ask, and `--yes` stands in for
    /// the confirmation. A non-interactive run missing a flag it needs is told
    /// which, rather than left waiting on input that will not come.
    Setup {
        #[command(flatten)]
        flags: RawSetup,
    },
    /// Report the versions in play.
    Version,
    /// List the forms this stack has, and what each one is for.
    ///
    /// A form says which part of the stack to run. They come from the stack rather
    /// than from lemonfiber, so a stack of your own names its own.
    ///
    /// Naming one says what starting it would come to — the services it holds, and
    /// anything your configuration leaves out — without starting anything.
    Forms {
        /// The forms to describe; none lists them all.
        forms: Vec<String>,
    },
    /// Start a form, or the union of several.
    Up {
        /// The forms to start; none starts everything the stack declares.
        forms: Vec<String>,
        /// Start only these services, leaving the rest of the form alone.
        #[arg(long = "service", value_name = "NAME")]
        services: Vec<String>,
    },
    /// Stop and remove what a form started.
    Down {
        /// The forms to stop; none stops everything the stack declares.
        forms: Vec<String>,
        /// Stop only these services, leaving the rest of the form running.
        #[arg(long = "service", value_name = "NAME")]
        services: Vec<String>,
        /// Let anything still downloading finish before stopping.
        #[arg(long)]
        wait: bool,
        /// Stop without asking about anything still downloading.
        #[arg(long, conflicts_with = "wait")]
        yes: bool,
    },
    /// Make these forms the active set, leaving shared services running.
    ///
    /// Only what falls outside the new shape is stopped. A service the old shape
    /// and the new one both hold keeps running rather than being restarted, so a
    /// download in flight is not interrupted to change the stack around it.
    Switch {
        /// The forms to switch to.
        #[arg(required = true)]
        forms: Vec<String>,
    },
    /// Restart services without touching the rest.
    Restart {
        /// The form holding them.
        form: String,
        /// The services to restart; none restarts the whole form.
        services: Vec<String>,
    },
    /// Fetch newer images without applying them.
    Pull {
        /// The forms whose images to fetch.
        #[arg(required = true)]
        forms: Vec<String>,
    },
    /// Report what each service is actually doing.
    Ps {
        /// The forms to report on; none reports the whole stack.
        forms: Vec<String>,
    },
    /// Show what services are saying.
    Logs {
        /// The services to read; none reads them all.
        services: Vec<String>,
        /// Read only the services a form declares.
        #[arg(long, value_name = "FORM")]
        form: Vec<String>,
        /// Keep reading as new lines arrive.
        #[arg(long, short)]
        follow: bool,
        /// Read them on a screen that can be scrolled back and filtered.
        #[arg(long, conflicts_with = "follow")]
        watch: bool,
        /// How many existing lines to begin with.
        #[arg(long, default_value_t = 50)]
        tail: u32,
    },
    /// Read or change one setting.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Choose how good your media should look, in plain language.
    Quality {
        #[command(subcommand)]
        action: QualityCommand,
    },
    /// Run the checks that prove the stack is doing what it should.
    Doctor {
        /// Run only one category of check, such as `vpn`.
        #[arg(long, value_name = "CATEGORY", conflicts_with = "fix")]
        only: Option<String>,
        /// Include the checks that disturb the running system.
        #[arg(long)]
        disruptive: bool,
        /// Answer a warning about a choice — `vpn.unprotected`, say — so it stops
        /// leading. Only something this run warns about can be answered.
        #[arg(long, value_name = "CHECK", conflicts_with = "fix")]
        accept: Option<String>,
        #[command(flatten)]
        mending: Mending,
    },
    /// Guard the data location while forms run, stopping them if it disappears.
    Watch {
        /// The forms to stop if the data location is lost.
        #[arg(required = true)]
        forms: Vec<String>,
    },
    /// Follow one show or film across the services — "where is my show?".
    ///
    /// Searched for the way you would name it, not by an internal id. Reports how far
    /// it got and, where it plainly stopped, why. A show is reported season by season:
    /// how many episodes are here, and what each one that is not is waiting on.
    Trace {
        /// The show or film to follow, named as you would say it.
        #[arg(required = true)]
        term: Vec<String>,
        /// Narrow to one season, instead of every season of the show.
        #[arg(long)]
        season: Option<u32>,
    },
    /// Show what the household asked for, and where each request stands.
    ///
    /// Grouped by whoever asked, in the words they would use rather than the
    /// services' own. Each named request links to its full trace.
    Household {
        /// Narrow to one member, named the way you would say it.
        #[arg(long)]
        member: Option<String>,
    },
    /// Add one thing, end to end, and watch every step of it happen.
    ///
    /// The walk a first run is offered: search the indexers, grab a release, download
    /// it, import it, and see it appear in the library — narrated as it goes, so that
    /// afterwards you know what your stack does because you watched it do it. If any
    /// link is broken this is where it shows, with the step named and a way out.
    ///
    /// Name something, or name nothing and be suggested something likely to work.
    Walkthrough {
        /// What to add, named as you would say it.
        item: Vec<String>,
    },
    /// Say what one of this product's words means.
    ///
    /// A report explains the words it used underneath itself, in a sentence. This is
    /// the longer form, for somebody who wants it — nothing needs it in order to act,
    /// which is the difference between an explanation offered and one imposed.
    Explain {
        /// The word, as you would say it.
        word: Vec<String>,
    },
    /// List the items whose downloads are stuck — the landing point for "N stuck", each
    /// named so `lemonfiber trace` follows it on its own.
    Stuck,
    /// Wire the stack's services to each other, idempotently.
    Seed,
    /// Adopt your current edits as lemonfiber's expected state.
    ///
    /// A value you changed by hand reports as drift until you adopt it; once
    /// adopted it is kept across future seeds and restores. Wires what is missing
    /// as a seed does, and promotes every drifted value to yours.
    Adopt,
    /// Put the stack back to lemonfiber's own state, reverting every edit you made.
    ///
    /// The opposite of adopt: it discards your hand-edits to the stack files and
    /// restores lemonfiber's own. Because it throws work away, it names exactly what
    /// will be lost and does nothing until `--confirm` — run it once to see the diffs,
    /// again with `--confirm` to reset.
    Reset {
        /// Go ahead and revert, having seen what will be lost.
        #[arg(long)]
        confirm: bool,
    },
    /// Back up your configuration to an archive, so it stops being precious.
    Backup {
        /// Back up one service's configuration instead of the whole stack.
        #[arg(long, value_name = "SERVICE")]
        service: Option<String>,
    },
    /// Gather everything a person helping you would ask for, with every value not
    /// named safe replaced by a stand-in.
    ///
    /// A bare run writes nothing. It collects, redacts, and reads the result back
    /// looking for anything that still resembles a credential, then says what the
    /// bundle would hold and how large it is — so the decision to make a file worth
    /// attaching to a public thread is taken after seeing what goes in it. Run it
    /// again with `--write` to produce it.
    ///
    /// Nothing is ever sent anywhere. The bundle is written here and stays here.
    Support(Asked),
    /// Restore your configuration from a backup archive.
    ///
    /// Verifies the archive and lists what it holds before anything is
    /// overwritten. A restore onto a different data root is refused until
    /// `--repoint` accepts moving it to this machine's.
    Restore {
        /// The archive to restore from.
        archive: PathBuf,
        /// Accept re-pointing to this machine's data root where it differs.
        #[arg(long)]
        repoint: bool,
    },
}

/// What `doctor` was asked to put right.
///
/// Declared here beside the subcommand rather than in the surface that reads them, as every
/// other set of flags is — a flag added to one list and not the other is a flag that
/// silently does nothing.
#[derive(Debug, Args)]
pub(crate) struct Mending {
    #[command(flatten)]
    pub(crate) fixing: Fixing,
    /// Put back what the last repair changed.
    ///
    /// Asked for the same way a repair is, because it is the same errand read
    /// backwards. It reverses that one repair and nothing else: the wiring lemonfiber
    /// seeded and the choices your first run wrote are left where they are.
    #[arg(long, conflicts_with = "fix")]
    pub(crate) undo: bool,
}

impl Mending {
    /// Whether this run was asked to change anything at all, forwards or back.
    ///
    /// Asked as one question so the caller deciding between looking and acting does not
    /// have to know which combination of flags amounts to acting.
    pub(crate) fn acts(&self) -> bool {
        self.fixing.fix || self.undo
    }
}

/// How much of the putting-right was agreed to in advance.
///
/// Apart from `--undo` because these are two errands rather than four settings: the three
/// here describe one run that changes things forward, and each is meaningless without the
/// first of them.
#[derive(Debug, Args)]
pub(crate) struct Fixing {
    /// Offer to put right what lemonfiber can, asking about each first.
    ///
    /// A plain run only looks. This one says what each repair would do and what else
    /// changes if it does, and waits to be told.
    #[arg(long)]
    pub(crate) fix: bool,
    /// Carry the repairs out without asking, having decided in advance.
    #[arg(long, requires = "fix")]
    pub(crate) yes: bool,
    /// Include the checks that disturb the running system while repairing.
    ///
    /// Named apart from the field it sits beside: `doctor` already has a `--disruptive`,
    /// and clap keys an argument by the field name unless told otherwise — so two flags
    /// that read differently on the command line would be one argument underneath.
    #[arg(id = "fix-disruptive", long = "fix-disruptive", requires = "fix")]
    pub(crate) disruptive: bool,
}

/// What a support bundle was asked for.
///
/// Declared as one thing rather than as six loose values on a variant, because the same
/// six would otherwise be written out again by whoever passes them on — and a flag added
/// to one list and not the other is a flag that silently does nothing.
#[derive(Debug, Args)]
pub(crate) struct Asked {
    /// Produce the bundle, having seen what it would hold.
    #[arg(long)]
    pub(crate) write: bool,
    /// Where to write it, instead of into this directory.
    #[arg(long, value_name = "PATH")]
    pub(crate) out: Option<PathBuf>,
    /// How many log lines to take from each service.
    #[arg(long, value_name = "LINES", default_value_t = LINES)]
    pub(crate) logs: u32,
    /// Include media filenames, which are replaced by default.
    #[arg(long)]
    pub(crate) filenames: bool,
    /// Show one setting as it is, named exactly as the bundle names it.
    ///
    /// Repeatable, and refused without `--confirm` on the same run: a flag that
    /// publishes a credential is not one to honour because it turned up on a
    /// command line somebody copied.
    #[arg(long, value_name = "SETTING")]
    pub(crate) reveal: Vec<String>,
    /// Confirm showing the settings named by `--reveal`.
    #[arg(long)]
    pub(crate) confirm: bool,
}

/// What to do with settings.
#[derive(Debug, Subcommand)]
pub(crate) enum QualityCommand {
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

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigAction {
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

#[cfg(test)]
mod tests {
    use super::{Cli, Mending, Request};
    use clap::Parser;

    /// What `doctor` was asked to do about what it finds, for one command line.
    ///
    /// Answered through the parser rather than by building the flags by hand, because the
    /// question being asked is what a person typing this actually gets — including the
    /// combinations the parser is meant to refuse. Nothing for a line that is not a
    /// `doctor` run, or that the parser turns away.
    fn doctoring(args: &[&str]) -> Option<(bool, Mending)> {
        match Cli::try_parse_from(args).ok()?.command? {
            Request::Doctor {
                disruptive,
                mending,
                ..
            } => Some((disruptive, mending)),
            _ => None,
        }
    }

    /// The forms a command line names, or nothing for a line that names none — a line
    /// the parser turns away included, since a refused line named nothing either.
    fn named_forms(args: &[&str]) -> Option<Vec<String>> {
        match Cli::try_parse_from(args).ok()?.command? {
            Request::Forms { forms } => Some(forms),
            _ => None,
        }
    }

    /// Asking what forms there are and asking what one of them would do are the same
    /// word, told apart by what follows it — so the parser has to keep both open.
    #[test]
    fn asking_for_the_forms_and_asking_about_one_are_the_same_word() {
        assert_eq!(named_forms(&["lemonfiber", "forms"]), Some(Vec::new()));
        assert_eq!(
            named_forms(&["lemonfiber", "forms", "tv"]),
            Some(vec!["tv".to_owned()])
        );
        // Composition is asked about exactly as it is started.
        assert_eq!(
            named_forms(&["lemonfiber", "forms", "full", "proxy"]),
            Some(vec!["full".to_owned(), "proxy".to_owned()])
        );
        // A profile is an implementation detail, and no surface takes one.
        assert_eq!(
            named_forms(&["lemonfiber", "forms", "--profile", "media"]),
            None
        );
        assert_eq!(named_forms(&["lemonfiber", "version"]), None);
    }

    /// Looking and acting are told apart by what was asked for, not by which flag carries
    /// it: a run that reverses a repair changes as much as one that makes it.
    #[test]
    fn a_run_that_changes_something_is_told_from_one_that_only_looks() {
        let acts = |args: &[&str]| doctoring(args).map(|(_, mending)| mending.acts());

        assert_eq!(acts(&["lemonfiber", "doctor"]), Some(false));
        assert_eq!(acts(&["lemonfiber", "doctor", "--fix"]), Some(true));
        assert_eq!(acts(&["lemonfiber", "doctor", "--undo"]), Some(true));
        // The question is doctor's alone — every other command already says what it does.
        assert_eq!(acts(&["lemonfiber", "seed"]), None);
    }

    /// Repairing and reversing a repair in one run is not a thing to guess the order of,
    /// so it is refused at the parser rather than resolved somewhere further in.
    #[test]
    fn repairing_and_reversing_at_once_is_refused() {
        assert!(doctoring(&["lemonfiber", "doctor", "--fix", "--undo"]).is_none());
    }

    /// Two flags that read differently on the command line must be two arguments
    /// underneath. `doctor` has a `--disruptive` of its own, and a repairing run has
    /// `--fix-disruptive`; keyed by field name they would collide, and the one that lost
    /// would silently do nothing.
    #[test]
    fn disturbing_the_stack_while_repairing_is_its_own_flag() {
        let disturbs =
            |args: &[&str]| doctoring(args).map(|(all, mending)| (all, mending.fixing.disruptive));

        assert_eq!(
            disturbs(&["lemonfiber", "doctor", "--disruptive"]),
            Some((true, false))
        );
        assert_eq!(
            disturbs(&["lemonfiber", "doctor", "--fix", "--fix-disruptive"]),
            Some((false, true))
        );
    }
}
