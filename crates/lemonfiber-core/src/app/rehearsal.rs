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

use crate::error::{Amiss, Problem, Remedy, Severity, State};

use super::command::{Asking, Keeping, Linking, MigrateAction};
use super::disturbance::Situation;
use super::engine::Waiting;
use super::plugins;
use super::setup::SetupAction;
use super::{repair, restore, update, Command, Ctx};

use crate::error::codes::rehearse::CANNOT;

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

use crate::error::codes::rehearse::NOT_YET;

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
    /// Which situation it puts the stack in, where it is one of the verbs that start
    /// or stop services. `None` means unstated rather than free: setup, an uninstall
    /// and a restore take something away too, and what bounds each of them is a
    /// different subsystem's answer.
    pub disturbs: Option<Situation>,
}

impl Asked {
    /// The same answer, putting the stack in `situation` while it runs.
    #[must_use]
    pub const fn disturbing(self, situation: Situation) -> Self {
        Self {
            disturbs: Some(situation),
            ..self
        }
    }
}

/// A command that changes nothing, so the rehearsal is the command.
const fn reads(named: &'static str) -> Asked {
    Asked {
        named,
        rehearsal: Rehearsal::Reads,
        disturbs: None,
    }
}

/// A command that reports what it would do and stops short of doing it.
const fn reports(named: &'static str) -> Asked {
    Asked {
        named,
        rehearsal: Rehearsal::Reports,
        disturbs: None,
    }
}

/// A command whose effect cannot be known without producing it.
const fn cannot(named: &'static str, why: &'static str) -> Asked {
    Asked {
        named,
        rehearsal: Rehearsal::Cannot(why),
        disturbs: None,
    }
}

/// What each command is: the name an operator knows it by, what a rehearsal of it
/// comes to, and what it takes away while it runs.
///
/// One table for all three, so a command added to [`Command`] is described once.
/// Exhaustive on purpose, and the reason this is a function rather than a field on
/// each handler: a handler carrying its own answer can be added without one, and
/// nothing would say so. Here, the build says so.
///
/// The sub-actions that are split are split because the split is real — reading a
/// credential and rotating one arrive as the same command, and refusing the flag on
/// the read would be refusing it on something that changes nothing.
#[must_use]
pub const fn asked(command: &Command) -> Asked {
    match command {
        // Reads. Nothing here reaches for anything it could put back.
        Command::Version => reads("version"),
        Command::Forms | Command::Preview { .. } => reads("forms"),
        Command::ConfigGet { .. } => reads("config get"),
        Command::ConfigShow => reads("config"),
        Command::History => reads("history"),
        Command::Status { .. } => reads("ps"),
        Command::Stuck => reads("stuck"),
        Command::FrontDoor => reads("front-door"),
        Command::Explain { .. } => reads("explain"),
        Command::Glossary => reads("glossary"),
        Command::Clients => reads("clients"),
        Command::Catalogue => reads("catalogue"),
        Command::Wiring(Linking::Read) => reads("wiring"),
        Command::Outbound => reads("outbound"),
        Command::Provenance => reads("provenance"),
        Command::Stored => reads("stored"),
        Command::Plugins(plugins::Asked::Installed) => reads("plugin installed"),
        Command::Archives => reads("archives"),
        Command::Migrate(MigrateAction::Survey) => reads("migrate"),
        Command::Credentials(Asking::Read | Asking::Reveal { .. }) => reads("credentials"),
        Command::Hosting(Keeping::Read) => reads("hosting"),
        Command::Setup(SetupAction::Where) => reads("setup --status"),
        Command::Support { write: false, .. } => reads("support"),
        // Its own doc comment is the verdict: it replaces nothing, and what it answers
        // with is the command for whichever tool owns the copy that is running. It was
        // refusing the flag it did not need to refuse, which costs an operator a run
        // and teaches them the flag is unreliable. The one thing a rehearsal holds
        // back is the note it keeps of having asked, so that a question does not move
        // the day the next real run is due to ask on.
        Command::SelfUpdate { .. } => reads("update self"),

        // A search asks an indexer, and the answer exists only once it has been
        // asked. Refused rather than reported because the asking is the part with a
        // cost: an indexer allows a fixed number of queries a day, and one spent on
        // a rehearsal is one the operator no longer has.
        Command::Trace {
            searching: true, ..
        } => cannot("trace --search", A_SEARCH_IS_THE_ANSWER),
        Command::Trace { .. } => reads("trace"),

        // The killswitch check *is* the disruption. It takes the tunnel's own route
        // down inside the container and watches what the rest of the stack does,
        // which is the only way to find out whether anything leaks. A rehearsal of it
        // could report the intent and nothing about the outcome, and the outcome is
        // the whole of what was asked for.
        Command::Doctor {
            disruptive: true, ..
        } => cannot("doctor --disruptive", THE_CHECK_IS_THE_DISRUPTION),
        Command::Doctor { accept: None, .. } => reads("doctor"),
        Command::Doctor { .. } => reports("doctor --accept"),

        // Same shape, for the same reason: a walk adds an item, waits for the stack to
        // do something with it, and reports what actually happened at each stage. What
        // it would report is what the stack did, and a rehearsal has no stack doing
        // anything.
        Command::Walkthrough { .. } => cannot("walkthrough", THE_WALK_IS_THE_OBSERVATION),

        // Reports. Each of these builds the report it would have filled in and stops
        // before the step it cannot take back.
        Command::Up { .. } => reports("up").disturbing(Situation::Starting),
        // A boot runs the same start in the middle by calling it, and is held to the same
        // clock: what it was prepared to wait is what somebody reads back afterwards to
        // understand why a four-in-the-morning start gave up when it did.
        Command::AtBoot => reports("up --at-boot").disturbing(Situation::Starting),
        Command::Start { .. } => reports("start").disturbing(Situation::Starting),
        Command::Down {
            wait: Waiting::ForTheDownloads,
            ..
        } => reports("down").disturbing(Situation::StoppingAfterDownloads),
        Command::Down { .. } => reports("down").disturbing(Situation::Stopping),
        Command::Halt { .. } => reports("stop").disturbing(Situation::Stopping),
        Command::Switch { .. } => reports("switch").disturbing(Situation::Switching),
        Command::Restart { .. } => reports("restart").disturbing(Situation::Restarting),
        Command::Pull { .. } => reports("pull"),
        Command::ConfigSet(_) => reports("config set"),
        // Everything it changes is settled before anything is touched: the manifest
        // is read, what the install decides is settled, and where every one of its
        // writes lands is derived without a disk under it. A rehearsal does all of
        // that, states it, and stops short of carrying it out — so what it reports is
        // what the real run reports rather than a summary of it.
        Command::Plugins(plugins::Asked::Install { .. }) => reports("plugin install"),
        // The same, read backwards. What a removal puts back is judged before a byte of
        // it is touched — the rollback layer's own judgement, which is the whole of
        // what can be known without acting — and what it would leave with nothing
        // filling it is a fact about the record rather than about a machine mid-run.
        // Taking a plugin off stops its containers through the same engine stop, held to
        // the same grace.
        Command::Plugins(plugins::Asked::Remove { .. }) => {
            reports("plugin remove").disturbing(Situation::Stopping)
        }
        // Both of those at once, as the one account the update is. What goes back is the
        // rollback layer's judgement and what comes on is the install's settled writes
        // and declared proofs, and neither needs anything touched to be known.
        // The version installed comes off and another comes on in its place, held to the
        // same settle wait a switch is.
        Command::Plugins(plugins::Asked::Update { .. }) => {
            reports("plugin update").disturbing(Situation::Switching)
        }
        Command::Wiring(Linking::Fill(_)) => reports("wiring fill"),
        Command::Quality(_) => reports("quality"),
        Command::Alerts(_) => reports("alerts"),
        Command::QualityMusic { .. } => reports("quality music"),
        Command::Household { .. } => reports("household"),
        Command::Held { .. } => reports("held"),
        Command::Allowing(_) => reports("allow"),
        Command::Deciding(_) => reports("decide"),
        Command::Expiring(_) => reports("expiring"),
        Command::Hosting(_) => reports("hosting"),
        Command::Invite { .. } => reports("invite"),
        Command::Reissue { .. } => reports("reissue"),
        Command::Forget { .. } => reports("forget"),
        Command::Space { .. } => reports("space"),
        Command::StopSeeding { .. } => reports("stop-seeding"),
        Command::Bandwidth(_) => reports("bandwidth"),
        Command::Uninstall(_) => reports("uninstall"),
        // A guard is the one command with no ending of its own, so a rehearsal of it
        // cannot be the command run with the last step left out — it would hold the
        // terminal until the drive was pulled. What it reports instead is the watch it
        // would keep: the location, how often it would look, and the invocation it
        // would run the moment that location went. The invocation is the lifecycle
        // stop's own, built by the same path a real watch builds it with.
        Command::Watch { .. } => reports("watch"),
        // Which changes would go back, and which of them need a service that is
        // answering. The judgement is already made before anything is touched, because
        // a run goes back whole or not at all — so the report a rehearsal wants is the
        // one this command has already formed by the time it would act.
        Command::Undo { .. } => reports("undo"),
        // Which credential would be replaced, where its value lives, and what would
        // still need doing before every consumer held the new one. No replacement is
        // generated: a value minted to describe a rotation is a secret that exists
        // because somebody asked a question, and it would have to go somewhere.
        Command::Credentials(_) => reports("credentials --rotate"),
        // What a capture would hold, how large it would be, and the exact path it
        // would be written to — read off the same room check a real capture makes
        // before it writes anything.
        Command::Backup { .. } => reports("backup"),

        // The eight whose yes a rehearsal takes back. Each already answers twice —
        // unconfirmed it says what it would do, confirmed it does it — so the report a
        // rehearsal wants is the one it already gives, in the same words. See
        // [`unconfirmed`].
        Command::Migrate(_) => reports("migrate"),
        Command::Remove { .. } => reports("remove"),
        Command::QualityUpgrade { .. } => reports("quality upgrade"),
        Command::Repair { .. } => reports("doctor --fix"),
        Command::Reset { .. } => reports("reset"),
        Command::Update(_) => reports("update"),
        Command::Restore { .. } => reports("restore"),
        Command::Support { .. } => reports("support --write"),

        // Setup is split where the split is real, the way `credentials` and `doctor`
        // are. Reading where the walk stands changes nothing; every other step records
        // something — an answer into the resumable progress file, or the whole of the
        // configuration — and each reports what it would record instead of recording
        // it. The answers gathered so far are the report either way, so a rehearsal is
        // the same walk with the file left alone.
        Command::Setup(_) => reports("setup"),

        // One pass over one graph, reported per connection: the field, what the service
        // holds now, and what would be pushed. The three-way reconcile every driver
        // already reads a connection through is what produces that, so the gate sits at
        // each write inside those same drivers rather than above the pass — a rehearsal
        // stopped at the door would have nothing to say, and one that surveyed
        // separately would be a second opinion about what lemonfiber intends.
        //
        // Adopting is the same survey in the other direction: nothing is written to any
        // service, and what a real run would move is lemonfiber's record of what it
        // expects, so a rehearsal of it names the values that would be taken on.
        Command::Seed => reports("seed"),
        Command::Adopt => reports("adopt"),
    }
}

/// The command as this run should carry it out.
///
/// A real run carries what was asked. A rehearsal carries the same thing with the
/// operator's go-ahead taken back — see [`unconfirmed`] for why that is the whole of
/// what a rehearsal of those commands needs to be.
#[must_use]
pub fn carried(command: Command, ctx: &Ctx) -> Command {
    if ctx.dry_run {
        unconfirmed(command)
    } else {
        command
    }
}

/// The same command with the operator's go-ahead withheld.
///
/// Eight commands here already answer twice: unconfirmed they say what they would do,
/// confirmed they do it. The unconfirmed answer *is* the rehearsal — the same report,
/// in the same words, filled in by the same code path — so a rehearsal takes the yes
/// back rather than eight handlers each learning a second way to say what they already
/// say. A second way is a second thing to keep true, and the one nobody exercises is
/// the one that stops being true.
///
/// Not exhaustive, and that is deliberate: this is the mechanism, not the decision.
/// What a rehearsal of a command *means* is decided in [`asked`], which the compiler
/// checks, and a command that claims to report while its go-ahead is not taken back
/// here fails `a_rehearsal_changes_nothing` against a real disk.
#[must_use]
fn unconfirmed(command: Command) -> Command {
    match command {
        Command::Reset { .. } => Command::Reset { confirm: false },
        Command::Remove { name, .. } => Command::Remove {
            name,
            confirm: false,
        },
        Command::QualityUpgrade { .. } => Command::QualityUpgrade { confirm: false },
        Command::Migrate(MigrateAction::Act { mode, .. }) => Command::Migrate(MigrateAction::Act {
            mode,
            confirmed: false,
        }),
        Command::Update(asked) => Command::Update(update::Asked {
            confirm: false,
            ..asked
        }),
        Command::Restore {
            archive, repoint, ..
        } => Command::Restore {
            archive,
            repoint,
            consent: restore::Consent::List,
        },
        Command::Repair { disruptive, .. } => Command::Repair {
            consent: repair::Consent::Offer,
            disruptive,
        },
        // A support bundle is asked for twice by the same word: without `--write` it
        // says what one would hold and where it would land, with it there is a file.
        // The description is the rehearsal, and it is the better command to be asked
        // for it — an operator deciding whether to produce the archive wants the size
        // and the path at the one moment the answer can still change what they do.
        Command::Support { wanted, dest, .. } => Command::Support {
            write: false,
            wanted,
            dest,
        },
        // Everything else either changes nothing, reports for itself, or refuses the
        // flag outright — none of which a withheld confirmation would change.
        other => other,
    }
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
    verdict(&asked(command))
}

/// What one of the four answers comes to, given what was asked.
///
/// Apart from the run that reaches it, because one of the four is an answer no
/// command carries today. `Untaught` is the escape hatch a command added tomorrow
/// gets: the match over every command is exhaustive, so whoever adds one has to
/// choose a verdict, and this is the one that says "not yet" out loud rather than
/// quietly rehearsing something that would act. Every command has since been taught,
/// which leaves the arm shipped and unreachable through `permitted` — and a rule
/// nothing can enter is a rule nobody has checked. Taking the answer rather than the
/// command is what lets it be handed one.
///
/// Public for that reason and only that reason. The arm is reachable nowhere
/// inside this crate, and a test in a `#[cfg(test)]` module would enter the copy
/// built for tests while the copy that ships stayed unentered — which is a rule
/// checked in a build nobody runs.
///
/// # Errors
///
/// Returns the [`Problem`] that refuses `--dry-run` where what was asked cannot be
/// rehearsed, or has not been taught to report what it would do.
pub fn verdict(asked: &Asked) -> Result<(), Box<Problem>> {
    match asked.rehearsal {
        Rehearsal::Reads | Rehearsal::Reports => Ok(()),
        Rehearsal::Cannot(why) => Err(Box::new(refused(asked, why))),
        Rehearsal::Untaught => Err(Box::new(not_taught_yet(asked.named))),
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
///
/// Takes the name rather than the whole of what was asked, which is what lets a test
/// in `tests/` reach it. It is unreachable through [`permitted`] — every command has
/// been taught — so the copy in the shipped build is a function no run enters, and a
/// function no run enters is counted against every covered line beside it. Reaching
/// it from outside the crate is what exercises the copy that ships.
#[must_use]
pub fn not_taught_yet(named: &str) -> Problem {
    Problem::new(
        NOT_YET,
        Severity::Error,
        format!("`{named}` does not rehearse yet"),
        "Nothing was done. This command changes things and has not yet been taught to \
         say what it would change, so it refuses the flag rather than accepting it and \
         going ahead — which is what it used to do."
            .to_owned(),
        Remedy::new(format!(
            "Run `lemonfiber {named}` without `--dry-run` when you mean it"
        )),
    )
    .lies_in(Amiss::Asking)
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
