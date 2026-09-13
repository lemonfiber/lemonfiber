//! What a rehearsal means, command by command.
//!
//! `--dry-run` is one flag over a surface of fifty-odd commands, and until this
//! existed it was a boolean each handler was trusted to read. Sixteen of them never
//! read it at all, and a handler that does not read it does not fail — it performs
//! the write and reports it as a rehearsal, which is the one outcome worse than not
//! offering the flag at all. An absent flag errors and the operator finds out; an
//! accepted one that does nothing takes a decision they explicitly declined to take.
//!
//! Reading it late is the same defect wearing a better disguise. Every lifecycle
//! command wrote the whole embedded stack to disk before it could build the
//! invocation it would report, and the gate against running Compose sat below that —
//! so the seven commands held up as the ones that got this right had each already
//! written the stack out by the time they decided not to run anything.
//!
//! So the decision is taken here instead, once per command, in a match the compiler
//! checks. A new command does not compile until somebody has said which of the four
//! answers below it gives, and [`super::dispatch`] acts on the answer before the
//! handler is reached. Forgetting is not a thing that can happen quietly: it is a
//! build failure while a command is being added, and a refusal afterwards.
//!
//! The four answers are deliberately not three. "Cannot be rehearsed" and "has not
//! been taught to rehearse yet" both refuse the flag, and an operator reading one
//! message wants to know which they are looking at — the first will not change, and
//! the second is a row on a board somebody is working through. Collapsing them would
//! turn the temporary into the permanent by making them indistinguishable.
//!
//! What a rehearsal must *say* is not decided here. It is said in the same words the
//! real run uses, by the same code path, because there is no second path: a handler
//! reports the report it would have filled in and stops short of the one irreversible
//! step. See `.docs/architecture/rehearsal.md`.

use lemonfiber_ports::error::{Amiss, Code, Problem, Remedy, Severity, State};

use super::command::{Asking, Keeping, MigrateAction};
use super::setup::SetupAction;
use super::{Command, Ctx};

/// The flag cannot be honoured by this command, and never will be.
const CANNOT: Code = Code::new("REHEARSE-1");

/// Why a search cannot be rehearsed.
///
/// Beside the other two rather than inside the arm that gives it: three sentences of
/// the same kind, said to an operator who has met one of them before, are worth
/// reading together — and an arm that is a pattern and a name reads as a decision
/// rather than as a paragraph.
const A_SEARCH_IS_THE_ANSWER: &str = "a search is the indexer's answer, which does not \
     exist until the indexer has been asked — and asking spends one of the queries the \
     operator's subscription allows that day";

/// Why the disruptive checks cannot be rehearsed.
const THE_CHECK_IS_THE_DISRUPTION: &str = "the disruptive checks find out what happens \
     by making it happen — a killswitch that has not been tested is a killswitch nobody \
     knows about, and there is nothing to predict from";

/// Why a walkthrough cannot be rehearsed.
const THE_WALK_IS_THE_OBSERVATION: &str = "a walkthrough is an end-to-end observation — \
     what it reports is what this stack actually did with a real item, which cannot be \
     known without asking it to";

/// The flag is not honoured by this command yet.
const NOT_YET: Code = Code::new("REHEARSE-2");

/// What a rehearsal means for one command.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Rehearsal {
    /// Nothing to rehearse: the command changes nothing, so the rehearsal *is* the
    /// command and runs exactly as it always does.
    Reads,
    /// Reported rather than carried out. The handler builds the report it would have
    /// filled in and stops short of the step that cannot be taken back.
    Reports,
    /// The effect cannot be known without producing it, so there is nothing honest to
    /// report. The flag is refused, with this as the reason.
    Cannot(&'static str),
    /// State-changing, and not taught to report yet. The flag is refused rather than
    /// ignored, which is the whole of the difference this module exists to make.
    Untaught,
}

impl Rehearsal {
    /// Why this command cannot be rehearsed, where that is the answer it gives.
    ///
    /// For a surface that wants to explain a refusal in its own words rather than
    /// render the one the core wrote — the reason is the part that is worth having,
    /// and asking for it should not mean matching on a verdict to find out there is
    /// nothing to ask about.
    #[must_use]
    pub const fn why(self) -> Option<&'static str> {
        match self {
            Self::Cannot(why) => Some(why),
            Self::Reads | Self::Reports | Self::Untaught => None,
        }
    }
}

/// What was asked for, and what a rehearsal of it comes to.
///
/// One value rather than two lookups because they are one decision: the name is what
/// a refusal has to say, and a name kept in a second table is a name that stops
/// matching the verdict beside it.
pub struct Asked {
    /// The command as an operator names it.
    pub named: &'static str,
    /// What a rehearsal of it means.
    pub rehearsal: Rehearsal,
}

/// What a rehearsal of this command comes to.
///
/// Exhaustive on purpose, and the reason this is a function rather than a field on
/// each handler: a handler carrying its own answer can be added without one, and
/// nothing would say so. Here, the build says so.
///
/// The sub-actions that are split are split because the split is real — reading a
/// credential and rotating one arrive as the same command, and refusing the flag on
/// the read would be refusing it on something that changes nothing.
#[must_use]
pub fn asked(command: &Command) -> Asked {
    let (named, rehearsal) = match command {
        // Reads. Nothing here reaches for anything it could put back.
        Command::Version => ("version", Rehearsal::Reads),
        Command::Forms | Command::Preview { .. } => ("forms", Rehearsal::Reads),
        Command::ConfigGet { .. } => ("config get", Rehearsal::Reads),
        Command::ConfigShow => ("config", Rehearsal::Reads),
        Command::History => ("history", Rehearsal::Reads),
        Command::Ps { .. } => ("ps", Rehearsal::Reads),
        Command::Stuck => ("stuck", Rehearsal::Reads),
        Command::FrontDoor => ("front-door", Rehearsal::Reads),
        Command::Explain { .. } => ("explain", Rehearsal::Reads),
        Command::Glossary => ("glossary", Rehearsal::Reads),
        Command::Clients => ("clients", Rehearsal::Reads),
        Command::Outbound => ("outbound", Rehearsal::Reads),
        Command::Stored => ("stored", Rehearsal::Reads),
        Command::Archives => ("archives", Rehearsal::Reads),
        Command::Migrate(MigrateAction::Survey) => ("migrate", Rehearsal::Reads),
        Command::Credentials(Asking::Read | Asking::Reveal { .. }) => {
            ("credentials", Rehearsal::Reads)
        }
        Command::Hosting(Keeping::Read) => ("hosting", Rehearsal::Reads),
        Command::Setup(SetupAction::Where) => ("setup --status", Rehearsal::Reads),
        Command::Support { write: false, .. } => ("support", Rehearsal::Reads),

        // A search asks an indexer, and the answer exists only once it has been
        // asked. Refused rather than reported because the asking is the part with a
        // cost: an indexer allows a fixed number of queries a day, and one spent on
        // a rehearsal is one the operator no longer has.
        Command::Trace {
            searching: true, ..
        } => ("trace --search", Rehearsal::Cannot(A_SEARCH_IS_THE_ANSWER)),
        Command::Trace { .. } => ("trace", Rehearsal::Reads),

        // The killswitch check *is* the disruption. It takes the tunnel's own route
        // down inside the container and watches what the rest of the stack does,
        // which is the only way to find out whether anything leaks. A rehearsal of it
        // could report the intent and nothing about the outcome, and the outcome is
        // the whole of what was asked for.
        Command::Doctor {
            disruptive: true, ..
        } => (
            "doctor --disruptive",
            Rehearsal::Cannot(THE_CHECK_IS_THE_DISRUPTION),
        ),
        Command::Doctor { accept: None, .. } => ("doctor", Rehearsal::Reads),
        Command::Doctor { .. } => ("doctor --accept", Rehearsal::Untaught),

        // Same shape, for the same reason: a walk adds an item, waits for the stack to
        // do something with it, and reports what actually happened at each stage. What
        // it would report is what the stack did, and a rehearsal has no stack doing
        // anything.
        Command::Walkthrough { .. } => (
            "walkthrough",
            Rehearsal::Cannot(THE_WALK_IS_THE_OBSERVATION),
        ),

        // Reports. Each of these builds the report it would have filled in and stops
        // before the step it cannot take back.
        Command::Up { .. } => ("up", Rehearsal::Reports),
        Command::Start { .. } => ("start", Rehearsal::Reports),
        Command::Down { .. } => ("down", Rehearsal::Reports),
        Command::Halt { .. } => ("stop", Rehearsal::Reports),
        Command::Switch { .. } => ("switch", Rehearsal::Reports),
        Command::Restart { .. } => ("restart", Rehearsal::Reports),
        Command::Pull { .. } => ("pull", Rehearsal::Reports),
        Command::ConfigSet(_) => ("config set", Rehearsal::Reports),
        Command::Quality(_) => ("quality", Rehearsal::Reports),
        Command::Alerts(_) => ("alerts", Rehearsal::Reports),
        Command::QualityMusic { .. } => ("quality music", Rehearsal::Reports),
        Command::Household { .. } => ("household", Rehearsal::Reports),
        Command::Allowing(_) => ("allow", Rehearsal::Reports),
        Command::Deciding(_) => ("decide", Rehearsal::Reports),
        Command::Expiring(_) => ("expiring", Rehearsal::Reports),
        Command::Hosting(_) => ("hosting", Rehearsal::Reports),
        Command::Invite { .. } => ("invite", Rehearsal::Reports),
        Command::Reissue { .. } => ("reissue", Rehearsal::Reports),
        Command::Forget { .. } => ("forget", Rehearsal::Reports),
        Command::Space { .. } => ("space", Rehearsal::Reports),
        Command::StopSeeding { .. } => ("stop-seeding", Rehearsal::Reports),
        Command::Bandwidth(_) => ("bandwidth", Rehearsal::Reports),
        Command::Uninstall(_) => ("uninstall", Rehearsal::Reports),

        // Untaught. Each changes something and reports it as though it had been asked
        // about, so each refuses the flag until it has been taught to report instead.
        // A guard is held open until the data location is lost, and a rehearsal of it
        // cannot wait for that — so what it would have to report is the watch it would
        // keep and what it would do when the moment came, which is a report it does not
        // have yet. Until it does, a rehearsal refuses rather than holding a terminal
        // open for a week.
        Command::Watch { .. } => ("watch", Rehearsal::Untaught),
        Command::Migrate(_) => ("migrate", Rehearsal::Untaught),
        Command::Remove { .. } => ("remove", Rehearsal::Untaught),
        Command::QualityUpgrade { .. } => ("quality upgrade", Rehearsal::Untaught),
        Command::Repair { .. } => ("doctor --fix", Rehearsal::Untaught),
        Command::Undo { .. } => ("undo", Rehearsal::Untaught),
        Command::Credentials(_) => ("credentials --rotate", Rehearsal::Untaught),
        Command::SelfUpdate { .. } => ("update self", Rehearsal::Untaught),
        Command::Seed => ("seed", Rehearsal::Untaught),
        Command::Adopt => ("adopt", Rehearsal::Untaught),
        Command::Reset { .. } => ("reset", Rehearsal::Untaught),
        Command::Setup(_) => ("setup", Rehearsal::Untaught),
        Command::Update(_) => ("update", Rehearsal::Untaught),
        Command::Backup { .. } => ("backup", Rehearsal::Untaught),
        Command::Support { .. } => ("support --write", Rehearsal::Untaught),
        Command::Restore { .. } => ("restore", Rehearsal::Untaught),
    };
    Asked { named, rehearsal }
}

/// Whether this run may go ahead, or the refusal to give instead of running it.
///
/// Consulted by [`super::dispatch`] before the handler is reached, which is the whole
/// of what makes the verdict above binding: a handler asked to decide for itself can
/// be written without deciding, and what that looks like from outside is the write
/// happening and the report calling it a rehearsal.
///
/// # Errors
///
/// Returns the [`Problem`] that refuses `--dry-run` where this command cannot be
/// rehearsed, or has not been taught to report what it would do.
pub fn permitted(command: &Command, ctx: &Ctx) -> Result<(), Box<Problem>> {
    if !ctx.dry_run {
        return Ok(());
    }
    let asked = asked(command);
    match asked.rehearsal {
        Rehearsal::Reads | Rehearsal::Reports => Ok(()),
        Rehearsal::Cannot(why) => Err(Box::new(refused(&asked, why))),
        Rehearsal::Untaught => Err(Box::new(not_taught_yet(&asked))),
    }
}

/// The refusal a command gives when it cannot rehearse what was asked of it.
///
/// Said here rather than at each handler for the reason the verdict is decided here:
/// a sentence written once is a sentence that stays the same across fifty commands,
/// and an operator who has met it on one recognises it on the next.
#[must_use]
fn refused(asked: &Asked, why: &'static str) -> Problem {
    Problem::new(
        CANNOT,
        Severity::Error,
        format!("`{}` cannot be rehearsed", asked.named),
        format!(
            "Nothing was done. {why}, so a rehearsal of this would be a report with \
             nothing in it that a rehearsal could have found out."
        ),
        Remedy::new(format!(
            "Run `lemonfiber {}` without `--dry-run` when you mean it",
            asked.named
        )),
    )
    .lies_in(Amiss::Asking)
    .in_state(State::Guided)
}

/// The refusal a command gives when it changes things and has not been taught to
/// report what it would change.
///
/// Separate from [`refused`] because the two are separate facts about the world, and
/// an operator can act on the difference: this one is a gap somebody is closing, and
/// the message says so rather than implying a limitation that is not there.
#[must_use]
fn not_taught_yet(asked: &Asked) -> Problem {
    Problem::new(
        NOT_YET,
        Severity::Error,
        format!("`{}` does not rehearse yet", asked.named),
        "Nothing was done. This command changes things and has not yet been taught to \
         say what it would change, so it refuses the flag rather than accepting it and \
         going ahead — which is what it used to do."
            .to_owned(),
        Remedy::new(format!(
            "Run `lemonfiber {}` without `--dry-run` when you mean it",
            asked.named
        )),
    )
    .lies_in(Amiss::Asking)
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests {
    use super::{asked, not_taught_yet, refused, Rehearsal};
    use crate::app::command::{Asking, Keeping, MigrateAction};
    use crate::app::setup::SetupAction;
    use crate::app::Command;

    /// The two refusals are told apart by their code, which is what an operator
    /// searches for and what a surface keys off.
    #[test]
    fn the_two_refusals_carry_codes_of_their_own() {
        let untaught = asked(&Command::Seed);
        let never = asked(&Command::Walkthrough { item: None });
        // Asked through `why` rather than through `matches!`, which expands to a match
        // whose second arm is only taken when the assertion is about to fail — a region
        // no passing run enters and one the coverage gate counts.
        assert!(
            never.rehearsal.why().is_some(),
            "a walkthrough should refuse the flag outright"
        );
        assert_ne!(
            not_taught_yet(&untaught).code,
            refused(&never, "the reason it gives").code,
            "a gap being closed and a limitation that will not change read alike"
        );
    }

    /// A refusal says which command refused, because an operator running a script
    /// sees the message without the command beside it.
    #[test]
    fn a_refusal_names_the_command_it_refused() {
        let one = asked(&Command::Seed);
        assert!(
            not_taught_yet(&one).summary.contains("seed"),
            "a refusal that does not name the command leaves the operator guessing"
        );
    }

    /// Reading a credential and rotating one arrive as the same command, and only one
    /// of them changes anything. A verdict taken at the outer variant would refuse the
    /// flag on a read.
    #[test]
    fn reading_a_credential_is_not_refused_the_way_rotating_one_is() {
        assert!(asked(&Command::Credentials(Asking::Read)).rehearsal == Rehearsal::Reads);
        assert!(
            asked(&Command::Credentials(Asking::Rotate {
                credential: "qbittorrent".to_owned(),
            }))
            .rehearsal
                == Rehearsal::Untaught
        );
    }

    /// A trace, asked with and without a live search.
    fn tracing(searching: bool) -> Command {
        Command::Trace {
            term: "anything".to_owned(),
            season: None,
            searching,
        }
    }

    /// A doctor run, disruptive or not, acknowledging nothing.
    fn examining(disruptive: bool) -> Command {
        Command::Doctor {
            narrowing: crate::doctor::Narrowing::Suite,
            disruptive,
            accept: None,
        }
    }

    /// A support bundle, written out or only described.
    fn bundling(write: bool) -> Command {
        Command::Support {
            write,
            wanted: crate::app::bundle::Wanted::default(),
            dest: crate::app::support::Destination::Kept,
        }
    }

    /// The three that refuse the flag for good say so, the read half of each command
    /// that shares a name with one does not, and the reason reaches the operator.
    ///
    /// Driven rather than read, because the two halves of `trace` and of `doctor` are
    /// one word to an operator and two arms here, and an arm nothing reaches is an arm
    /// whose pattern could be wrong in either direction without anything saying so.
    #[test]
    fn what_cannot_be_rehearsed_is_told_from_the_read_beside_it() {
        for (command, refuses) in [
            (tracing(true), true),
            (tracing(false), false),
            (examining(true), true),
            (examining(false), false),
            (Command::Walkthrough { item: None }, true),
            (bundling(false), false),
        ] {
            let asked = asked(&command);
            assert!(
                asked.rehearsal.why().is_some() == refuses,
                "{command:?} was read as the wrong one of the two"
            );
            // The reason reaches the operator rather than staying in the source, which
            // is the difference between refusing and refusing usefully.
            if let Some(why) = asked.rehearsal.why() {
                assert!(refused(&asked, why).meaning.contains(why), "{command:?}");
            }
        }
    }

    /// The same split, on the three other commands that carry a read and a write under
    /// one name.
    #[test]
    fn every_command_that_carries_both_is_split_on_which_it_is() {
        for (command, expected) in [
            (Command::Migrate(MigrateAction::Survey), Rehearsal::Reads),
            (Command::Hosting(Keeping::Read), Rehearsal::Reads),
            (Command::Setup(SetupAction::Where), Rehearsal::Reads),
            (Command::Setup(SetupAction::Apply), Rehearsal::Untaught),
        ] {
            assert!(
                asked(&command).rehearsal == expected,
                "{command:?} was read as the wrong half of what it carries"
            );
        }
    }
}
