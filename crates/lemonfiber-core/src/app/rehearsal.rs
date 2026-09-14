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
use super::{repair, restore, update, Command, Ctx};

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
        Command::Catalogue => ("catalogue", Rehearsal::Reads),
        Command::Outbound => ("outbound", Rehearsal::Reads),
        Command::Provenance => ("provenance", Rehearsal::Reads),
        Command::Stored => ("stored", Rehearsal::Reads),
        Command::Archives => ("archives", Rehearsal::Reads),
        Command::Migrate(MigrateAction::Survey) => ("migrate", Rehearsal::Reads),
        Command::Credentials(Asking::Read | Asking::Reveal { .. }) => {
            ("credentials", Rehearsal::Reads)
        }
        Command::Hosting(Keeping::Read) => ("hosting", Rehearsal::Reads),
        Command::Setup(SetupAction::Where) => ("setup --status", Rehearsal::Reads),
        Command::Support { write: false, .. } => ("support", Rehearsal::Reads),
        // Its own doc comment is the verdict: it replaces nothing, and what it answers
        // with is the command for whichever tool owns the copy that is running. It was
        // refusing the flag it did not need to refuse, which costs an operator a run
        // and teaches them the flag is unreliable. The one thing a rehearsal holds
        // back is the note it keeps of having asked, so that a question does not move
        // the day the next real run is due to ask on.
        Command::SelfUpdate { .. } => ("update self", Rehearsal::Reads),

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
        Command::Doctor { .. } => ("doctor --accept", Rehearsal::Reports),

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
        Command::AtBoot => ("up --at-boot", Rehearsal::Reports),
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
        // A guard is the one command with no ending of its own, so a rehearsal of it
        // cannot be the command run with the last step left out — it would hold the
        // terminal until the drive was pulled. What it reports instead is the watch it
        // would keep: the location, how often it would look, and the invocation it
        // would run the moment that location went. The invocation is the lifecycle
        // stop's own, built by the same path a real watch builds it with.
        Command::Watch { .. } => ("watch", Rehearsal::Reports),
        // Which changes would go back, and which of them need a service that is
        // answering. The judgement is already made before anything is touched, because
        // a run goes back whole or not at all — so the report a rehearsal wants is the
        // one this command has already formed by the time it would act.
        Command::Undo { .. } => ("undo", Rehearsal::Reports),
        // Which credential would be replaced, where its value lives, and what would
        // still need doing before every consumer held the new one. No replacement is
        // generated: a value minted to describe a rotation is a secret that exists
        // because somebody asked a question, and it would have to go somewhere.
        Command::Credentials(_) => ("credentials --rotate", Rehearsal::Reports),
        // What a capture would hold, how large it would be, and the exact path it
        // would be written to — read off the same room check a real capture makes
        // before it writes anything.
        Command::Backup { .. } => ("backup", Rehearsal::Reports),

        // The eight whose yes a rehearsal takes back. Each already answers twice —
        // unconfirmed it says what it would do, confirmed it does it — so the report a
        // rehearsal wants is the one it already gives, in the same words. See
        // [`unconfirmed`].
        Command::Migrate(_) => ("migrate", Rehearsal::Reports),
        Command::Remove { .. } => ("remove", Rehearsal::Reports),
        Command::QualityUpgrade { .. } => ("quality upgrade", Rehearsal::Reports),
        Command::Repair { .. } => ("doctor --fix", Rehearsal::Reports),
        Command::Reset { .. } => ("reset", Rehearsal::Reports),
        Command::Update(_) => ("update", Rehearsal::Reports),
        Command::Restore { .. } => ("restore", Rehearsal::Reports),
        Command::Support { .. } => ("support --write", Rehearsal::Reports),

        // Setup is split where the split is real, the way `credentials` and `doctor`
        // are. Reading where the walk stands changes nothing; every other step records
        // something — an answer into the resumable progress file, or the whole of the
        // configuration — and each reports what it would record instead of recording
        // it. The answers gathered so far are the report either way, so a rehearsal is
        // the same walk with the file left alone.
        Command::Setup(_) => ("setup", Rehearsal::Reports),

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
        Command::Seed => ("seed", Rehearsal::Reports),
        Command::Adopt => ("adopt", Rehearsal::Reports),
    };
    Asked { named, rehearsal }
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
fn verdict(asked: &Asked) -> Result<(), Box<Problem>> {
    match asked.rehearsal {
        Rehearsal::Reads | Rehearsal::Reports => Ok(()),
        Rehearsal::Cannot(why) => Err(Box::new(refused(asked, why))),
        Rehearsal::Untaught => Err(Box::new(not_taught_yet(asked))),
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
    use super::{
        asked, carried, not_taught_yet, refused, repair, restore, update, verdict, Asked,
        Rehearsal, A_SEARCH_IS_THE_ANSWER, THE_CHECK_IS_THE_DISRUPTION,
        THE_WALK_IS_THE_OBSERVATION,
    };
    use crate::app::command::{
        AlertAction, Arranged, Asking, BandwidthAsked, Chosen, Decision, Keeping, MigrateAction,
        QualityAction, Removing, Setting,
    };
    use crate::app::engine::Waiting;
    use crate::app::setup::SetupAction;
    use crate::app::Command;

    /// A command that has not been taught to rehearse is refused rather than run.
    ///
    /// Nothing carries this verdict today — every command that changes something has
    /// since been taught to say what it would change — so the answer is handed over
    /// directly rather than asked of a command. That is the point of it: this is the
    /// escape hatch whoever adds the next command gets, and an escape hatch nothing
    /// has ever been through is one nobody knows the shape of. What it must do is
    /// refuse, and refuse under its own code: an operator who typed `--dry-run` and
    /// was quietly run for real is the failure the whole flag exists to prevent, and
    /// "this one has not been taught yet" is a different thing to be told from "this
    /// one cannot be rehearsed at all".
    #[test]
    fn a_command_nobody_has_taught_to_rehearse_is_refused_under_its_own_code() {
        let untaught = Asked {
            named: "invent",
            rehearsal: Rehearsal::Untaught,
        };

        // Carried as a `Result` rather than opened with a `let ... else`. The else
        // arm is a region no passing run enters, and the coverage gate counts it
        // against this file — the same reason the test below reaches for
        // `reasoning` instead of `matches!`.
        let answer = verdict(&untaught).map_err(|refusal| refusal.code);

        assert_eq!(answer, Err(not_taught_yet(&untaught).code));
        assert_ne!(
            answer,
            Err(refused(&untaught, THE_WALK_IS_THE_OBSERVATION).code),
            "not taught yet and cannot be rehearsed are different things to be told"
        );
    }

    /// The two refusals are told apart by their code, which is what an operator
    /// searches for and what a surface keys off.
    #[test]
    fn the_two_refusals_carry_codes_of_their_own() {
        // `not_taught_yet` builds its refusal from the command's name rather than from
        // its verdict, so any command names one — which matters, because nothing
        // carries `Untaught` today. What is asked here is about the two sentences, not
        // about which command happens to be on the board when somebody reads them.
        let untaught = asked(&Command::Seed);
        let never = asked(&Command::Walkthrough { item: None });
        // Read with `reasoning` rather than with `matches!`, which expands to a match
        // whose second arm is only taken when the assertion is about to fail — a region
        // no passing run enters and one the coverage gate counts.
        assert!(
            reasoning(never.rehearsal).is_some(),
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
    /// of them changes anything. A verdict taken at the outer variant would hold the
    /// read to a rule written for the write.
    #[test]
    fn reading_a_credential_is_not_read_the_way_rotating_one_is() {
        assert!(asked(&Command::Credentials(Asking::Read)).rehearsal == Rehearsal::Reads);
        assert!(
            asked(&Command::Credentials(Asking::Rotate {
                credential: "qbittorrent".to_owned(),
            }))
            .rehearsal
                == Rehearsal::Reports
        );
    }

    /// The reason a verdict gives, where it gives one.
    ///
    /// Here rather than on `Rehearsal` because here is the only place that asks. A
    /// verdict carries its reason and `permitted` reads it out of the pattern, so an
    /// accessor beside it was a second way to ask one question — and the shipped
    /// build never called it, which is a whole function's worth of lines nothing
    /// enters and the coverage gate counts.
    fn reasoning(rehearsal: Rehearsal) -> Option<&'static str> {
        match rehearsal {
            Rehearsal::Cannot(why) => Some(why),
            Rehearsal::Reads | Rehearsal::Reports | Rehearsal::Untaught => None,
        }
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
                reasoning(asked.rehearsal).is_some() == refuses,
                "{command:?} was read as the wrong one of the two"
            );
            // The reason reaches the operator rather than staying in the source, which
            // is the difference between refusing and refusing usefully.
            if let Some(why) = reasoning(asked.rehearsal) {
                assert!(refused(&asked, why).meaning.contains(why), "{command:?}");
            }
        }
    }

    /// Every command, against the answer a rehearsal of it gives.
    ///
    /// The match is exhaustive, so the compiler already refuses a new command with no
    /// verdict. What it cannot refuse is a *wrong* verdict, or a pattern catching more
    /// than it means — and an arm nothing drives is an arm whose pattern could be wrong
    /// in either direction with nothing to say so. Forty-five of these had never been
    /// read by anything.
    ///
    /// Here rather than beside the integration test that dispatches them, and the
    /// reason is not tidiness. This file is compiled twice — once into the binary and
    /// once for its own tests — and a table living in another crate leaves this copy's
    /// arms unentered however thoroughly the other copy is driven. The coverage gate
    /// counts both, which is how a module every line of which is reached came to read
    /// as ninety-four per cent.
    ///
    /// The sub-patterns are listed beside the variants they split, because the split is
    /// where the mistake lives: `Support { write: false }` and `Support { .. }` are one
    /// word to an operator and two arms here.
    ///
    /// One group below per verdict, and the verdict written once for the group rather
    /// than once per row. A row under the wrong heading used to be a row that still
    /// declared the right answer beside itself, and read correctly while sitting in the
    /// wrong place; now the heading is the answer.
    fn every_verdict() -> Vec<(Command, Rehearsal)> {
        fn under(commands: Vec<Command>, verdict: Rehearsal) -> Vec<(Command, Rehearsal)> {
            commands
                .into_iter()
                .map(|command| (command, verdict))
                .collect()
        }

        let mut every = under(reads(), Rehearsal::Reads);
        every.extend(refused_for_good());
        every.extend(under(reports(), Rehearsal::Reports));
        every.extend(under(untaught(), Rehearsal::Untaught));
        every
    }

    /// The commands a rehearsal runs exactly as it always does.
    ///
    /// Nothing here reaches for anything it could put back, so there is nothing to
    /// hold off on and no report to give in place of the work.
    fn reads() -> Vec<Command> {
        vec![
            Command::Version,
            Command::Forms,
            Command::Preview {
                forms: vec!["library".to_owned()],
            },
            Command::ConfigGet {
                key: "DATA_ROOT".to_owned(),
            },
            Command::ConfigShow,
            Command::History,
            Command::Ps { forms: Vec::new() },
            Command::Stuck,
            Command::FrontDoor,
            Command::Explain {
                word: "seeding".to_owned(),
            },
            Command::Glossary,
            Command::Clients,
            Command::Catalogue,
            Command::Outbound,
            Command::Provenance,
            Command::Stored,
            Command::Archives,
            Command::Migrate(MigrateAction::Survey),
            Command::Credentials(Asking::Read),
            Command::Credentials(Asking::Reveal {
                credential: "qbittorrent".to_owned(),
                confirmed: true,
            }),
            Command::Hosting(Keeping::Read),
            Command::Setup(SetupAction::Where),
            Command::SelfUpdate { to: None },
            bundling(false),
            tracing(false),
            examining(false),
        ]
    }

    /// The three that refuse the flag for good, each with its own reason.
    ///
    /// Listed with their reasons rather than under one verdict, because here the
    /// reason is the whole of the verdict: they differ in nothing else.
    fn refused_for_good() -> Vec<(Command, Rehearsal)> {
        vec![
            (tracing(true), Rehearsal::Cannot(A_SEARCH_IS_THE_ANSWER)),
            (
                examining(true),
                Rehearsal::Cannot(THE_CHECK_IS_THE_DISRUPTION),
            ),
            (
                Command::Walkthrough { item: None },
                Rehearsal::Cannot(THE_WALK_IS_THE_OBSERVATION),
            ),
        ]
    }

    /// The commands that report instead of acting.
    ///
    /// Each builds what it would have filled in and stops before the step it cannot
    /// take back.
    ///
    /// Three lists rather than one, because one of them outgrew the length rule and
    /// the seams were already written into it as comments. They are the three ways a
    /// command comes to rehearse: it always did, it was taught to, or it already had
    /// an answer for an unconfirmed run and a rehearsal is that answer with the yes
    /// taken back on the way in.
    fn reports() -> Vec<Command> {
        let mut every = always_reported();
        every.extend(taught_to_report());
        every.extend(answering_twice());
        every
    }

    /// The ones that reported before any of this: the flag was read where it mattered
    /// and the write was never reached.
    fn always_reported() -> Vec<Command> {
        vec![
            Command::Up { forms: Vec::new() },
            Command::Start {
                forms: Vec::new(),
                services: Vec::new(),
            },
            Command::Down {
                forms: Vec::new(),
                wait: Waiting::Never,
            },
            Command::Halt {
                forms: Vec::new(),
                services: Vec::new(),
            },
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
            Command::Restart {
                forms: Vec::new(),
                services: Vec::new(),
            },
            Command::Pull { forms: Vec::new() },
            Command::ConfigSet(Setting::to("DATA_ROOT", "/srv/library").agreed(true)),
            Command::Quality(QualityAction::Set {
                preset: crate::quality::Preset::Maximum,
                media_type: None,
                confirm: true,
            }),
            Command::Alerts(AlertAction::Set(crate::alert::Appetite::Everything)),
            Command::QualityMusic {
                format: crate::audio::Format::Lossless,
            },
            Command::Household { member: None },
            Command::Allowing(Chosen::default()),
            Command::Deciding(Decision {
                request: 1,
                answer: crate::app::Answer::LetThrough,
            }),
            Command::Expiring(Arranged::After(30)),
            Command::Hosting(Keeping::Install {
                what: crate::app::Hostable::Watch,
                forms: Vec::new(),
            }),
            Command::Invite {
                name: "ana".to_owned(),
                allowance: crate::app::Allowance::default(),
            },
            Command::Reissue {
                name: "ana".to_owned(),
            },
            Command::Forget { confirm: true },
            Command::Space { confirm: true },
            Command::StopSeeding {
                download: "anything".to_owned(),
                agreement: None,
            },
            Command::Bandwidth(BandwidthAsked {
                down: Some("20".to_owned()),
                ..BandwidthAsked::default()
            }),
            Command::Uninstall(Removing {
                tier: crate::uninstall::Tier::Configuration,
                confirm: true,
                agreement: None,
                waiting: Waiting::Never,
            }),
        ]
    }

    /// Taught to report rather than to act: each stops short of the write and says
    /// what the write would have been.
    fn taught_to_report() -> Vec<Command> {
        vec![
            examining_accepting(),
            Command::Watch { forms: Vec::new() },
            Command::Undo { run: None },
            Command::Credentials(Asking::Rotate {
                credential: "qbittorrent".to_owned(),
            }),
            Command::Setup(SetupAction::Apply),
            Command::Backup { service: None },
        ]
    }

    /// The ones that answer twice.
    ///
    /// Here rather than under `Untaught` because the answer each gives unconfirmed is
    /// the report a rehearsal wants, and `carried` is what takes the yes back on the
    /// way in.
    fn answering_twice() -> Vec<Command> {
        vec![
            Command::Migrate(MigrateAction::Act {
                mode: crate::migration::mode::Mode::Adopt,
                confirmed: true,
            }),
            Command::Remove {
                name: "ana".to_owned(),
                confirm: true,
            },
            Command::QualityUpgrade { confirm: true },
            Command::Repair {
                consent: crate::app::repair::Consent::Standing,
                disruptive: false,
            },
            Command::Reset { confirm: true },
            Command::Update(crate::app::update::Asked {
                service: None,
                confirm: true,
                wait: Waiting::Never,
            }),
            Command::Restore {
                archive: crate::app::restore::Kept::Named("anything".to_owned()),
                repoint: false,
                consent: crate::app::restore::Consent::Standing,
            },
            bundling(true),
            Command::Seed,
            Command::Adopt,
        ]
    }

    /// The commands that change something and have not been taught to say what.
    ///
    /// Empty, and kept rather than deleted: the verdict it stands for is still one of
    /// the four, a command added tomorrow can still be given it, and a reader comparing
    /// this table with the match wants to see that nothing carries it rather than to
    /// find the row missing. `every_command_is_answered_the_way_the_table_says` reads
    /// it whatever it holds.
    fn untaught() -> Vec<Command> {
        Vec::new()
    }

    /// A doctor run acknowledging a finding, which is the third of its three arms.
    fn examining_accepting() -> Command {
        Command::Doctor {
            narrowing: crate::doctor::Narrowing::Suite,
            disruptive: false,
            accept: Some("storage.one-filesystem".to_owned()),
        }
    }

    /// Every command gives the verdict this module says it gives.
    #[test]
    fn every_command_is_answered_the_way_the_table_says() {
        // `then_some` rather than an `if` with a body, and the name built before
        // the comparison rather than inside it. A block that runs only when a row
        // disagrees is a block nothing enters while the table is right, and the
        // coverage gate counts regions: this test passing is exactly the condition
        // under which the old shape read as an unreached line. Naming all
        // sixty-five commands to use none of them costs nothing here.
        let wrong: Vec<String> = every_verdict()
            .into_iter()
            .filter_map(|(command, expected)| {
                let named = format!("{command:?}");
                (asked(&command).rehearsal != expected).then_some(named)
            })
            .collect();

        assert!(
            wrong.is_empty(),
            "these were read as a verdict other than the one declared for them: {wrong:?}"
        );
    }

    /// A rehearsal of the eight that answer twice is the answer they already give
    /// unconfirmed, so the go-ahead is taken back on the way in.
    #[test]
    fn a_rehearsal_carries_the_confirmable_commands_without_their_yes() {
        let rehearsing = crate::test_support::a_context().build().rehearsing();
        for asked in [
            Command::Reset { confirm: true },
            Command::Remove {
                name: "ana".to_owned(),
                confirm: true,
            },
            Command::QualityUpgrade { confirm: true },
            Command::Migrate(MigrateAction::Act {
                mode: crate::migration::mode::Mode::Adopt,
                confirmed: true,
            }),
            Command::Update(update::Asked {
                service: None,
                confirm: true,
                wait: Waiting::Never,
            }),
            Command::Restore {
                archive: restore::Kept::Named("anything".to_owned()),
                repoint: false,
                consent: restore::Consent::Standing,
            },
            Command::Repair {
                consent: repair::Consent::Standing,
                disruptive: false,
            },
            bundling(true),
        ] {
            let carried = carried(asked.clone(), &rehearsing);
            assert_ne!(carried, asked, "{asked:?} kept the yes it was given");
        }
    }

    /// And what a rehearsal of a support bundle carries is the read beside it, rather
    /// than merely something other than what was asked.
    ///
    /// Its own case because `assert_ne!` above is satisfied by any difference, and the
    /// difference that matters here is which of the command's two halves runs: the
    /// describing one, which is already classified as changing nothing.
    #[test]
    fn a_rehearsed_support_bundle_is_the_run_that_describes_one() {
        let rehearsing = crate::test_support::a_context().build().rehearsing();
        assert_eq!(carried(bundling(true), &rehearsing), bundling(false));
    }

    /// And a real run carries exactly what it was handed, because the withholding is
    /// what a rehearsal is rather than something the dispatcher does to everybody.
    #[test]
    fn a_real_run_carries_the_command_it_was_given() {
        let real = crate::test_support::a_context().build();
        let asked = Command::Reset { confirm: true };
        assert_eq!(carried(asked.clone(), &real), asked);
    }

    /// The same split, on the three other commands that carry a read and a write under
    /// one name.
    #[test]
    fn every_command_that_carries_both_is_split_on_which_it_is() {
        for (command, expected) in [
            (Command::Migrate(MigrateAction::Survey), Rehearsal::Reads),
            (Command::Hosting(Keeping::Read), Rehearsal::Reads),
            (Command::Setup(SetupAction::Where), Rehearsal::Reads),
            (Command::Setup(SetupAction::Apply), Rehearsal::Reports),
        ] {
            assert!(
                asked(&command).rehearsal == expected,
                "{command:?} was read as the wrong half of what it carries"
            );
        }
    }
}
